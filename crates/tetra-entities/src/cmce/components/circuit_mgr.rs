use std::collections::HashMap;

use tetra_config::bluestation::{CircuitState, CircuitStreamDest, CircuitStreamSrc, MleRoute, StackState, TetraCircuit};
use tetra_core::{TdmaTime, TetraAddress, TimeslotOwner, TxReporter, frames, multiframes};
use tetra_pdus::cmce::pdus::d_setup::DSetup;
use tetra_pdus::cmce::structs::cmce_circuit::CallId;
use tetra_saps::control::enums::communication_type::CommunicationType;

/// D-SETUP is sent once more within this many frames of circuit creation, as the back-up
/// transmission of ETSI Annex D Figure D.2.
const D_SETUP_REPEATS: i32 = 1;
/// Late entry D-SETUP interval for group calls, so a radio that missed the start can join.
const LATE_ENTRY_INTERVAL_TIMESLOTS: i32 = multiframes!(5);
/// Absolute circuit lifetime safety net, beyond the 5-minute call timeout T5m. Live calls
/// are normally torn down much earlier by the hangtime and release logic.
const CIRCUIT_EXPIRY_TIMESLOTS: i32 = 6 * 60 * 18 * 4; // 6 minutes

/// Work the circuit manager wants CMCE to carry out on this tick.
pub enum CircuitMgrCmd {
    /// Repeat the cached D-SETUP for a group call. Call id, usage marker, timeslot.
    SendDSetup(CallId, u8, u8),
    /// Circuit exceeded its absolute lifetime. Call id, timeslot.
    SendClose(CallId, u8),
}

/// The D-SETUP of a group call, kept so it can be repeated for late entry and turned into a
/// D-RELEASE at teardown. Holds the PDU itself and the transmit receipt of the last re-send,
/// which throttles the next one, so it stays out of the shared circuit view.
pub struct CachedSetup {
    pub pdu: DSetup,
    pub dest_addr: TetraAddress,
    pub receipt: Option<TxReporter>,
}

/// Everything needed to open, change and close a circuit. The timeslots, call id and usage
/// markers are assigned by the manager.
pub struct CircuitRequest {
    pub caller: u32,
    pub callee: u32,
    pub comm_type: CommunicationType,
    /// State the circuit starts in. A group call is through-connected at once, an individual
    /// call waits for the called party to answer.
    pub state: CircuitState,
    /// Duplex was negotiated on air.
    pub is_duplex: bool,
    /// Allocate a second local traffic channel so both parties can transmit at once. Only for
    /// a local duplex call: an over-Brew duplex call has the backend on the far side.
    pub second_channel: bool,
    /// The far party is reached over Brew instead of on air.
    pub over_brew: bool,
    pub hook_on_off: bool,
    pub is_local_origin: bool,
    pub is_mobile_terminated: bool,
    pub origin_brew_uuid: Option<uuid::Uuid>,
    pub caller_route: MleRoute,
    /// ISSI that starts out holding the floor, if any.
    pub floor: Option<u32>,
}

// Read/write interface for modifying circuits, only to be used by CMCE.
pub struct CircuitMgr {
    state: StackState,
    next_call_id: u16,
    next_usage_id: u8,
    dltime: TdmaTime,
    /// Cached D-SETUP PDUs for late-entry re-sends, keyed by call id.
    cached_setups: HashMap<CallId, CachedSetup>,
}

impl CircuitMgr {
    pub fn new(state: StackState) -> Self {
        Self {
            state,
            next_call_id: 4,
            next_usage_id: 4,
            dltime: TdmaTime::default(),
            cached_setups: HashMap::new(),
        }
    }

    /// Reserves a traffic timeslot from the cell-wide allocator, which Brew also draws from.
    fn take_timeslot(&self) -> Option<u8> {
        self.state.with_circuits(|c| c.allocator.allocate_any(TimeslotOwner::Cmce))
    }

    /// Returns a traffic timeslot to the cell-wide allocator.
    pub fn release_timeslot(&self, ts: u8) {
        self.state.with_circuits(|c| {
            if let Err(err) = c.allocator.release(TimeslotOwner::Cmce, ts) {
                tracing::warn!("CircuitMgr: failed to release timeslot ts={} err={:?}", ts, err);
            }
        })
    }

    /// Returns a clone of a circuit
    pub fn get_circuit_by_callid(&self, call_id: CallId) -> Option<TetraCircuit> {
        self.state.with_circuits(|x| x.get_circuit_by_callid(call_id).cloned())
    }

    /// Returns a clone of the first circuit matching the predicate.
    pub fn find_circuit<F>(&self, pred: F) -> Option<TetraCircuit>
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.state.with_circuits(|x| x.find_circuit(pred).cloned())
    }

    /// True if any circuit matches the predicate.
    pub fn any_circuit<F>(&self, pred: F) -> bool
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.state.with_circuits(|x| x.any_circuit(pred))
    }

    /// Returns clones of every circuit matching the predicate.
    pub fn find_circuits<F>(&self, pred: F) -> Vec<TetraCircuit>
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.state
            .with_circuits(|x| x.get_circuits().values().filter(|c| pred(c)).cloned().collect())
    }

    /// Mutates a circuit in place. Returns false if the call id is unknown.
    pub fn update_circuit<F>(&self, call_id: CallId, f: F) -> bool
    where
        F: FnOnce(&mut TetraCircuit),
    {
        let updated = self.state.with_circuits(|c| c.update_circuit_with(call_id, f));
        if !updated {
            tracing::warn!("CircuitMgr: update for unknown call_id {}", call_id);
        }
        updated
    }

    /// Moves a circuit to a new state and restarts its state timer.
    pub fn set_state(&self, call_id: CallId, state: CircuitState) {
        let now = self.dltime;
        self.update_circuit(call_id, |c| {
            c.state = state;
            c.t_state = now;
        });
    }

    /// Sets the floor holder of a circuit.
    pub fn set_floor(&self, call_id: CallId, floor: Option<u32>) {
        self.update_circuit(call_id, |c| c.floor = floor);
    }

    /// Caches the D-SETUP of a group call for late-entry re-sends and teardown.
    /// Can also be used to update pre-existing cached D-SETUP
    pub fn cache_setup(&mut self, call_id: CallId, pdu: DSetup, dest_addr: TetraAddress) {
        self.cached_setups.insert(
            call_id,
            CachedSetup {
                pdu,
                dest_addr,
                // Starts as None so the back-up send triggered on the next tick is not
                // throttled by this initial transmission.
                receipt: None,
            },
        );
    }

    pub fn get_setup(&self, call_id: CallId) -> Option<&CachedSetup> {
        self.cached_setups.get(&call_id)
    }

    pub fn get_setup_mut(&mut self, call_id: CallId) -> Option<&mut CachedSetup> {
        self.cached_setups.get_mut(&call_id)
    }

    pub fn take_setup(&mut self, call_id: CallId) -> Option<CachedSetup> {
        self.cached_setups.remove(&call_id)
    }

    fn get_next_call_id(&mut self) -> CallId {
        let call_id = self.next_call_id;
        self.next_call_id += 1;
        if self.next_call_id > 0x3FF {
            self.next_call_id = 1; // Wrap around, skip reserved zero value
        }
        call_id
    }

    /// Gets the next available usage ID. Checks that the usage ID is not already in use by another circuit.
    /// Wraps around at 63, skipping reserved values 0-3.
    pub fn get_next_usage_id(&mut self) -> u8 {
        let mut candidate = self.next_usage_id;
        let first_candidate = candidate;

        let current_usage_ids: Vec<u8> = self.state.with_circuits(|c| {
            let mut usage_ids = Vec::new();
            // for ts in 0..NUM_TIMESLOTS {
            for circuit in c.get_circuits().values() {
                usage_ids.push(circuit.usage_id);
            }
            usage_ids
        });

        loop {
            if !current_usage_ids.contains(&candidate) {
                self.next_usage_id = candidate + 1;
                if self.next_usage_id > 63 {
                    self.next_usage_id = 4;
                }
                return candidate;
            }

            // This usage id is still in use. Try next one.
            candidate += 1;
            if candidate > 63 {
                candidate = 4; // Wrap around, skip reserved values
            }

            // Check if exhausted. This should never happen.
            assert_ne!(candidate, first_candidate, "no free usage id");
        }
    }

    //     self.next_usage_id = candidate + 1;
    //     if self.next_usage_id > 63 {
    //         self.next_usage_id = 4;
    //     }
    //     candidate as u8
    // }

    /// Opens a circuit and reserves its traffic timeslot(s). Returns the call id, or None when
    /// no timeslot is free, in which case nothing has been reserved.
    ///
    /// Stream routing follows the media path. A group call and a local simplex call share one
    /// slot. A local duplex call uses two, cross-routed so each party's uplink feeds the other
    /// party's downlink: the calling party sits on the first slot (ul1/dl2) and the called party
    /// on the second (dl1/ul2). An over-Brew call has one local slot with the backend attached
    /// to its downlink.
    pub fn allocate_circuit(&mut self, req: CircuitRequest) -> Option<CallId> {
        // First slot carries the local calling party (or the called MS on a terminated call).
        let ts1 = self.take_timeslot()?;
        let ts2 = if req.second_channel {
            match self.take_timeslot() {
                Some(ts) => Some(ts),
                None => {
                    // Roll back so a failed duplex setup leaks nothing.
                    self.release_timeslot(ts1);
                    return None;
                }
            }
        } else {
            None
        };

        let ul1_source = CircuitStreamSrc::Local(Some(ts1));
        let dl1_source = if req.over_brew {
            CircuitStreamDest::LocalAndRemote(Some(ts1), req.origin_brew_uuid)
        } else {
            CircuitStreamDest::Local(Some(ts2.unwrap_or(ts1)))
        };
        let (ul2_source, dl2_source) = match ts2 {
            Some(ts2) => (Some(CircuitStreamSrc::Local(Some(ts2))), Some(CircuitStreamDest::Local(Some(ts1)))),
            None => (None, None),
        };

        let call_id = self.get_next_call_id();
        let usage_id = self.get_next_usage_id();
        let usage2_id = if ts2.is_some() { Some(self.get_next_usage_id()) } else { None };

        let circuit = TetraCircuit {
            state: req.state,
            t_state: self.dltime,
            call_id,
            usage_id,
            usage2_id,
            ul1_source,
            dl1_source,
            ul2_source,
            dl2_source,
            is_duplex: req.is_duplex,
            hook_on_off: req.hook_on_off,
            comm_type: req.comm_type,
            floor: req.floor,
            caller: req.caller,
            callee: req.callee,
            is_local_origin: req.is_local_origin,
            is_mobile_terminated: req.is_mobile_terminated,
            brew_origin_uuid: req.origin_brew_uuid,
            caller_route: req.caller_route,
            is_etee_encrypted: false,
            t_start: self.dltime,
        };

        // Sanity check on duplex local/remote relation between the two channels
        if circuit.has_second_channel() {
            assert!(
                circuit.ul1_source.is_local() == circuit.dl2_source.unwrap().has_local()
                    && circuit.ul2_source.unwrap().is_remote() == circuit.dl1_source.has_remote(),
                "duplex diagonal relations between source and dest for two channels do not hold"
            );
        };

        self.state.with_circuits(|c| c.put_circuit(circuit));
        Some(call_id)
    }

    /// Collapses a duplex call onto the calling party's slot, as ETSI 14.5.1.1.1 allows when the
    /// called MS cannot do duplex. Returns the freed second timeslot, which the caller still has
    /// to close towards UMAC. Returns None if there was no second channel.
    pub fn downgrade_duplex_to_simplex(&mut self, call_id: CallId) -> Option<u8> {
        self.state.with_circuits(|c| {
            let mut circuit = c.take_circuit(call_id).expect("call_id not found");
            if !circuit.has_second_channel() {
                c.put_circuit(circuit);
                return None;
            }
            let caller_ts = circuit.ul1_source.get_ts().expect("duplex circuit has a caller slot");
            let freed_ts = circuit.dl1_source.get_ts().expect("duplex circuit has a called slot");
            circuit.dl1_source = CircuitStreamDest::Local(Some(caller_ts));
            circuit.dl2_source = None;
            circuit.ul2_source = None;
            circuit.usage2_id = None;
            circuit.is_duplex = false;
            c.put_circuit(circuit);
            Some(freed_ts)
        })
    }

    /// Tears a circuit down: drops it from the global view, frees its traffic timeslot(s) and
    /// discards any cached D-SETUP. Returns the removed circuit, or None if the call id is
    /// unknown.
    pub fn destroy_circuit(&mut self, call_id: CallId) -> Option<TetraCircuit> {
        self.cached_setups.remove(&call_id);
        let circuit = self.state.with_circuits(|cs| cs.take_circuit(call_id));
        let Some(circuit) = circuit else {
            tracing::warn!("CircuitMgr: destroy for unknown call_id {}", call_id);
            return None;
        };
        if !circuit.is_releasing() {
            tracing::warn!(
                "circuit for call_id {} in wrong state for destruction: {:?}",
                call_id,
                circuit.state
            );
        }
        self.release_timeslot(circuit.dl_ts());
        if let Some(peer_ts) = circuit.peer_ts() {
            self.release_timeslot(peer_ts);
        }
        Some(circuit)
    }

    /// Per-tick housekeeping. Only acts on the first timeslot of each frame, so the D-SETUP
    /// schedule is counted in whole frames.
    pub fn tick_start(&mut self, dltime: TdmaTime) -> Option<Vec<CircuitMgrCmd>> {
        self.dltime = dltime;
        if dltime.t != 1 {
            return None;
        }

        let mut tasks: Option<Vec<CircuitMgrCmd>> = None;

        // Snapshot what the schedule needs, so the store is not borrowed while queueing work.
        let circuits: Vec<(CallId, u8, u8, bool, i32, bool)> = self.state.with_circuits(|c| {
            c.get_circuits()
                .values()
                .map(|x| {
                    (
                        x.call_id,
                        x.usage_id,
                        x.dl_ts(),
                        x.is_group_call(),
                        x.t_start.age(dltime),
                        x.is_releasing(),
                    )
                })
                .collect()
        });

        for (call_id, usage_id, ts, is_group, age, is_releasing) in circuits {
            // Safety net for a circuit that outlived every call timer.
            if age > CIRCUIT_EXPIRY_TIMESLOTS {
                tasks.get_or_insert_with(Vec::new).push(CircuitMgrCmd::SendClose(call_id, ts));
                continue;
            }

            // Only group calls are repeated: an individual call is point-to-point, so it has no
            // late joiners, and a circuit already in teardown must not be advertised again.
            if !is_group || is_releasing {
                continue;
            }

            // Back-up transmission shortly after setup (ETSI Annex D Figure D.2), then late
            // entry every few seconds. Ages are compared in frames because this only runs on
            // t == 1 while the circuit may have been created on any timeslot.
            let repeat = age < frames!(D_SETUP_REPEATS) || (age / 4) % (LATE_ENTRY_INTERVAL_TIMESLOTS / 4) == 0;
            if repeat {
                tracing::debug!("CircuitMgr: scheduling D-SETUP for call_id={} (age {} timeslots)", call_id, age);
                tasks
                    .get_or_insert_with(Vec::new)
                    .push(CircuitMgrCmd::SendDSetup(call_id, usage_id, ts));
            }
        }

        tasks
    }
}
