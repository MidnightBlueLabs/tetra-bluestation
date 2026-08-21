use tetra_config::bluestation::{CircuitMap, CircuitState, CircuitStreamDest, CircuitStreamSrc, NUM_TIMESLOTS, StackState, TetraCircuit};
use tetra_core::TdmaTime;
use tetra_pdus::cmce::structs::cmce_circuit::{CallId, CmceCircuit};
use tetra_saps::control::enums::communication_type::CommunicationType;

// #[derive(Debug, Clone, Default)]
// pub struct CircuitTimeslots {
//     dl1: Option<u8>,
//     ul1: Option<u8>,
//     dl2: Option<u8>, // Only for duplex
//     ul2: Option<u8>, // Only for duplex
// }

/// Placeholder, should remove
pub enum CircuitMgrCmd {
    SendDSetup(CallId, u8, u8), // call id, usage number, timeslot
    SendClose(CallId, CmceCircuit),
}

// Read/write interface for modifying circuits, only to be used by CMCE.
pub struct CircuitMgrNew {
    state: StackState,
    next_call_id: u16,
    next_usage_id: u8,
}

impl CircuitMgrNew {
    pub fn new(state: StackState) -> Self {
        Self {
            state,
            next_call_id: 4,
            next_usage_id: 4,
        }
    }

    /// Finds a ts that is free on both ul and dl
    fn find_free_ts(map: &CircuitMap) -> Option<u8> {
        for i in 2..NUM_TIMESLOTS {
            if map.dl[i].is_none() && map.ul[i].is_none() {
                // Free on ul and dl
                return Some(i as u8);
            }
        }
        // No ts was free
        None
    }

    /// Returns a clone of a circuit
    pub fn get_circuit_by_callid(&self, call_id: CallId) -> Option<TetraCircuit> {
        self.state.with_circuits(|x| x.get_circuit_by_callid(call_id).cloned())
    }

    /// Finds free timeslots for a simplex or duplex circuit for which no timeslots have yet been allocated.
    fn allocate_circuit_to_timeslots(&self, mut circuit: TetraCircuit) -> Option<TetraCircuit> {
        let mut map = self.state.with_circuits(|x| x.get_timeslot_map());

        if circuit.dl1_source.has_local() || circuit.ul1_source.is_local() {
            // Find ts free on both ul and dl, for safety and simplicyt
            // TODO optimize
            // Return None on failure
            let ts = Self::find_free_ts(&map)?;

            // Update passed mutable circuit struct and return struct
            if circuit.dl1_source.has_local() {
                circuit.dl1_source.set_ts(ts);
            };
            if circuit.ul1_source.is_local() {
                circuit.ul1_source.set_ts(ts);
            };

            // Flag as taken on dl and ul map (for if we need to find a 2nd ts for duplex)
            map.dl[ts as usize] = Some(circuit.call_id);
            map.ul[ts as usize] = Some(circuit.call_id);
        }

        // For duplex circuits, find a second ts
        // Unwraps never fail
        if circuit.is_duplex() && (circuit.dl2_source.unwrap().has_local() || circuit.ul2_source.unwrap().is_local()) {
            // Find ts free on both ul and dl, for safety and simplicyt
            // TODO optimize
            // Return None on failure
            let ts = Self::find_free_ts(&map)?;

            // Update passed mutable circuit struct and return struct.
            // Must go through as_mut(), as the stream enums are Copy and writing through
            // unwrap() on the Option would only mutate a temporary copy.
            if let Some(dl2) = circuit.dl2_source.as_mut()
                && dl2.has_local()
            {
                dl2.set_ts(ts);
            };
            if let Some(ul2) = circuit.ul2_source.as_mut()
                && ul2.is_local()
            {
                ul2.set_ts(ts);
            };

            // No further allocation follows, so the local map needs no update here.
        }
        Some(circuit)
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

    // /// Gets a circuit by its timeslot number.
    // pub fn get_circuit(&self, ts: u8) -> Option<&TetraCircuit> {
    //     if ts as usize >= NUM_TIMESLOTS {
    //         return None;
    //     }
    //     return self.circuits[ts as usize].as_ref();
    // }

    /// Allocates a new circuit, if a timeslot is available. Returns the timeslot number of the allocated circuit, or None if no timeslot is available.

    pub fn allocate_circuit(
        &mut self,
        caller: u32,
        callee: u32,
        comm_type: CommunicationType,
        ul1_source: CircuitStreamSrc,
        dl1_source: CircuitStreamDest,
        ul2_source: Option<CircuitStreamSrc>,  // Only for duplex
        dl2_source: Option<CircuitStreamDest>, // Only for duplex
        t_start: TdmaTime,
    ) -> Option<CallId> {
        let call_id = self.get_next_call_id();
        let usage_id = self.get_next_usage_id();
        let is_duplex = ul2_source.is_some(); // shortcut but it's asserted later
        let usage2_id = if is_duplex { Some(self.get_next_usage_id()) } else { None };

        let circuit = TetraCircuit {
            state: CircuitState::Setup(0),
            call_id,
            usage_id,
            usage2_id,
            ul1_source,
            dl1_source,
            ul2_source, // Only for duplex
            dl2_source, // Only for duplex
            comm_type,
            floor: if is_duplex { None } else { Some(caller) },
            caller,
            callee,
            is_etee_encrypted: false,
            t_start,
        };

        // Sanity check on duplex local/remote relation between the two channels
        if circuit.is_duplex() {
            assert!(
                circuit.ul1_source.is_local() == circuit.dl2_source.unwrap().has_local()
                    && circuit.ul2_source.unwrap().is_remote() == circuit.dl1_source.has_remote(),
                "duplex diagonal relations between source and dest for two channels do not hold"
            );
        };

        // Update our circuit with newly allocated timeslot(s), then store in global state
        let result = self.allocate_circuit_to_timeslots(circuit);
        if let Some(circuit) = result {
            self.state.with_circuits(|c| c.put_circuit(circuit));
            Some(call_id)
        } else {
            // Circuit could not be allocated
            None
        }
    }

    pub fn downgrade_duplex_to_simplex(&mut self, call_id: CallId) {
        self.state.with_circuits(|c| {
            let mut circuit = c.take_circuit(call_id).expect("call_id not found");
            assert!(circuit.is_duplex(), "can't downgrade non-duplex call");
            circuit.dl2_source = None;
            circuit.ul2_source = None;
            circuit.usage2_id = None;
            c.put_circuit(circuit);
        })
    }

    /// Destroys a circuit. Assumes circuit has been torn down, D-RELEASEs sent, etc.
    pub fn destroy_circuit(&mut self, call_id: CallId) -> TetraCircuit {
        self.state.with_circuits(|cs| {
            let circuit = cs.destroy_circuit_by_callid(call_id);
            match circuit.state {
                CircuitState::Releasing(_) => {
                    // Expected state
                }
                _ => {
                    tracing::warn!(
                        "circuit for call_id {} in wrong state for destruction: {:?}",
                        call_id,
                        circuit.state
                    );
                }
            }
            circuit
        })
    }
}
