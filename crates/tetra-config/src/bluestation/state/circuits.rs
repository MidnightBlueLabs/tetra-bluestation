use std::{collections::HashMap, usize};

use tetra_core::{SsiType, TdmaTime, TetraAddress, TimeslotAllocator};
use tetra_pdus::cmce::structs::cmce_circuit::CallId;
use tetra_saps::control::enums::communication_type::CommunicationType;

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Call was just set up, initial D-SETUPs are being sent out. Contains timeslots elapsed since initial setup
    /// Transitions:
    /// -> Alerting if the called party reports ringing
    /// -> Tx after 3 D-SETUPs sent TODO IMPLEMENT
    /// -> TxCeased if U-TX CEASED received TODO IMPLEMENT
    /// -> Releasing if U-RELEASE received TODO IMPLEMENT
    Setup(u32),
    /// Individual call only: the called party is ringing and has not answered yet.
    /// Both parties are still on the control channel.
    /// Transitions:
    /// -> Tx when the called party answers
    /// -> Releasing on release or no-answer timeout
    Alerting(u32),
    /// Call currently in TX, contains timeslots elapsed since tx segment started (including initial Setup if no TxCeased followed
    /// D-SETUP frames are emitted periodically
    /// Transitions:
    /// -> TxCeased if U-TX CEASED received TODO IMPLEMENT
    /// -> Releasing if U-RELEASE received TODO IMPLEMENT
    Tx(u32),
    /// Call TX ceased, contains timeslots elapsed since hangtime start
    /// Transitions:
    /// -> Tx if Brew receives new data TODO IMPLEMENT
    /// -> Tx if U TX DEMAND received and granted TODO IMPLEMENT
    /// -> Releasing if U-RELEASE received TODO IMPLEMENT
    TxCeased(u32),
    /// Call has been terminated, contains timeslots elapsed since the release started
    /// Transitions:
    /// -> <destroyed> after 3 D-RELEASEs sent TODO IMPLEMENT
    Releasing(u32),
}

#[derive(Debug, Clone, Copy)]
pub enum CircuitStreamSrc {
    Unknown,
    /// The data stream originates from local MS. Holds the local uplink timeslot number
    Local(Option<u8>),
    /// The data stream originates from a remote MS. Holds the remote Brew uuid
    Remote(Option<uuid::Uuid>),
}

impl CircuitStreamSrc {
    /// Gets timeslot (if Local aspect and ts is set)
    pub fn get_ts(&self) -> Option<u8> {
        match self {
            CircuitStreamSrc::Local(ts) => ts.clone(),
            _ => None,
        }
    }

    /// Gets uuid (if Remote aspect and uuid is set)
    pub fn get_uuid(&self) -> Option<uuid::Uuid> {
        match self {
            CircuitStreamSrc::Remote(uuid) => uuid.clone(),
            _ => None,
        }
    }

    pub fn set_ts(&mut self, ts: u8) {
        assert!(ts > 1, "invalid timeslot {}", ts);
        match self {
            CircuitStreamSrc::Local(cur_ts) => *cur_ts = Some(ts),
            _ => panic!("{:?} has no ts", self),
        }
    }

    pub fn set_uuid(&mut self, uuid: uuid::Uuid) {
        match self {
            CircuitStreamSrc::Remote(cur_uuid) => *cur_uuid = Some(uuid),
            _ => panic!("{:?} has no uuid", self),
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            CircuitStreamSrc::Local(_) => true,
            _ => false,
        }
    }

    pub fn is_remote(&self) -> bool {
        match self {
            CircuitStreamSrc::Remote(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum CircuitStreamDest {
    /// There is no known destination for this stream (anymore)
    /// May happen at call start or when the receiving MS disconnects
    Unknown,
    /// The data stream is destined for local MS(es)
    Local(Option<u8>),
    /// The data stream is destined for remote MS(es)
    Remote(Option<uuid::Uuid>),
    /// The data stream is destined for both local and remote MS(es). Holds the local downlink timeslot number and the remote Brew uuid
    /// Only possible for group calls, as individual calls are always either local or remote.
    LocalAndRemote(Option<u8>, Option<uuid::Uuid>),

    /// There currently are no listening parties
    NoListeners,
}

impl CircuitStreamDest {
    /// Gets timeslot (if Local aspect and ts is set)
    pub fn get_ts(&self) -> Option<u8> {
        match self {
            CircuitStreamDest::Local(ts) | CircuitStreamDest::LocalAndRemote(ts, _) => ts.clone(),
            _ => None,
        }
    }

    /// Gets uuid (if Remote aspect and uuid is set)
    pub fn get_uuid(&self) -> Option<uuid::Uuid> {
        match self {
            CircuitStreamDest::Remote(uuid) | CircuitStreamDest::LocalAndRemote(_, uuid) => uuid.clone(),
            _ => None,
        }
    }

    pub fn set_ts(&mut self, ts: u8) {
        assert!(ts > 1, "invalid timeslot {}", ts);
        match self {
            CircuitStreamDest::Local(cur_ts) | CircuitStreamDest::LocalAndRemote(cur_ts, _) => *cur_ts = Some(ts),
            _ => panic!("{:?} has no ts", self),
        }
    }

    pub fn set_uuid(&mut self, uuid: uuid::Uuid) {
        match self {
            CircuitStreamDest::Remote(cur_uuid) | CircuitStreamDest::LocalAndRemote(_, cur_uuid) => *cur_uuid = Some(uuid),
            _ => panic!("{:?} has no uuid", self),
        }
    }

    pub fn has_local(&self) -> bool {
        match self {
            CircuitStreamDest::Local(_) | CircuitStreamDest::LocalAndRemote(_, _) => true,
            _ => false,
        }
    }

    pub fn has_remote(&self) -> bool {
        match self {
            CircuitStreamDest::Remote(_) | CircuitStreamDest::LocalAndRemote(_, _) => true,
            _ => false,
        }
    }
}

/// MLE routing back to a local MS over its already established LLC link. Downlink PDUs that
/// must reach one specific MS (rather than a group broadcast) have to carry these.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MleRoute {
    pub handle: u32,
    pub link_id: u32,
    pub endpoint_id: u32,
}

impl MleRoute {
    pub fn new(handle: u32, link_id: u32, endpoint_id: u32) -> Self {
        Self {
            handle,
            link_id,
            endpoint_id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TetraCircuit {
    /// Current state of the call. Element contains data associated uniquely with this state, e.g. elapsed time in setup or hangtime in tx ceased.
    pub state: CircuitState,
    /// Time the circuit entered its current state. Drives hangtime, setup and release timers.
    pub t_state: TdmaTime,
    /// Source of the downlink stream. Can be local, remote or both, depending on who's subscribed.
    pub dl1_source: CircuitStreamDest,
    /// Source of the local or remote "uplink" stream.
    pub ul1_source: CircuitStreamSrc,

    /// Source of the duplex secondary downlink stream.
    /// Can be local, remote or both, depending on who's subscribed.
    pub dl2_source: Option<CircuitStreamDest>,
    /// Source of the duplex secondary local or remote "uplink" stream.
    pub ul2_source: Option<CircuitStreamSrc>,

    // Circuit mode; for now, only TchS (speech) is supported
    // pub circuit_mode_type: CircuitModeType,
    // Speech service, 0 = TETRA ACELP encoded speech, 1|2 = reserved, 3 = proprietary
    // pub speech_service: Option<u8>,
    /// Duplex as negotiated on air (ETSI 14.5.1.1.1). This is not the same as having a second
    /// local traffic channel: a duplex call whose far leg is reached over Brew has only one
    /// local slot. Use `has_second_channel()` for the timeslot question.
    pub is_duplex: bool,

    /// On/off hook signalling was selected, so the called party alerts before answering.
    pub hook_on_off: bool,

    /// Set to true for an individual call, false for a group call.
    pub comm_type: CommunicationType,

    /// Unique call ID
    pub call_id: u16,
    /// MAC layer usage ID, used to tie signalling data to this call
    pub usage_id: u8,
    /// Duplex channel MAC layer usage ID
    pub usage2_id: Option<u8>,

    /// ISSI that currently holds the floor. Always populated unless for duplex calls
    pub floor: Option<u32>,

    /// ISSI that opened the call. Does not equal the one who now has the floor!
    /// Zero for a call pushed in by the network, which has no local owner.
    pub caller: u32,
    /// ISSI or GSSI that was called
    pub callee: u32,

    /// The call was opened by a local MS. False when the network pushed it in over Brew.
    pub is_local_origin: bool,

    /// Mobile terminated over-Brew call: the network is the calling party and the local MS is
    /// the called party, so the on-air leg is `callee`, not `caller`.
    pub is_mobile_terminated: bool,

    /// Brew session of the party that opened the call. The stream sources carry the *current*
    /// Brew session, which the backend re-issues per speaker and which is cleared in hangtime,
    /// so this is kept separately for the final teardown notification.
    pub origin_brew_uuid: Option<uuid::Uuid>,

    /// MLE routing back to the local calling MS. Unset for a network originated call.
    pub caller_route: MleRoute,

    /// Call voice frames are E2EE encrypted
    pub is_etee_encrypted: bool,

    /// Time of original call start
    pub t_start: TdmaTime,
}

impl TetraCircuit {
    pub fn is_group_call(&self) -> bool {
        matches!(self.comm_type, CommunicationType::P2MpAcked | CommunicationType::P2Mp)
    }
    pub fn is_individual_call(&self) -> bool {
        matches!(self.comm_type, CommunicationType::P2p)
    }

    /// A second local traffic channel is allocated, so both parties can transmit at once and
    /// each uplink is cross-routed to the other party's downlink. Only a local duplex call has
    /// this: an over-Brew duplex call has one local slot and the backend on the other side.
    pub fn has_second_channel(&self) -> bool {
        let ret = self.dl2_source.is_some();
        // Some sanity checks
        if ret {
            assert!(self.is_individual_call(), "second channel but also group call");
            assert!(self.ul2_source.is_some(), "dl2_source set but ul2_source is None");
            assert!(self.is_duplex, "second channel but call is not duplex");
        } else {
            assert!(self.ul2_source.is_none(), "ul2_source set but dl2_source is None");
        }
        ret
    }

    /// The far party is reached over Brew (off-cell ISSI, PBX or phone number) rather than on air.
    pub fn is_over_brew(&self) -> bool {
        self.dl1_source.has_remote()
    }

    /// Brew session currently attached to this circuit, if any.
    pub fn brew_uuid(&self) -> Option<uuid::Uuid> {
        self.dl1_source.get_uuid()
    }

    pub fn caller_addr(&self) -> TetraAddress {
        TetraAddress::new(self.caller, SsiType::Issi)
    }

    pub fn callee_addr(&self) -> TetraAddress {
        let ssi_type = if self.is_group_call() { SsiType::Gssi } else { SsiType::Issi };
        TetraAddress::new(self.callee, ssi_type)
    }

    /// Local downlink timeslot. Every circuit this cell serves has one.
    pub fn dl_ts(&self) -> u8 {
        self.dl1_source.get_ts().expect("circuit has no local downlink timeslot")
    }

    /// Timeslot the calling party transmits on. For a group call with a network speaker the
    /// uplink is remote, so the local slot is taken from the downlink instead.
    pub fn ul_ts(&self) -> u8 {
        self.ul1_source.get_ts().unwrap_or_else(|| self.dl_ts())
    }

    /// Timeslot the called party is on. Equal to `ul_ts()` unless a second channel is allocated.
    pub fn callee_ts(&self) -> u8 {
        self.dl1_source.get_ts().unwrap_or_else(|| self.ul_ts())
    }

    /// MAC usage marker of the called party's channel.
    pub fn callee_usage_id(&self) -> u8 {
        self.usage2_id.unwrap_or(self.usage_id)
    }

    /// Second local traffic channel timeslot, if one is allocated.
    pub fn peer_ts(&self) -> Option<u8> {
        self.dl2_source.and_then(|d| d.get_ts())
    }

    /// Local on-air party of an individual call and the slot it is on. A mobile terminated
    /// over-Brew call has the called MS on air, everything else the calling MS.
    pub fn local_leg(&self) -> (TetraAddress, u8) {
        let ts = self.ul_ts();
        if self.is_mobile_terminated {
            (self.callee_addr(), ts)
        } else {
            (self.caller_addr(), ts)
        }
    }

    /// True while D-SETUP is out but the called party has not alerted or answered.
    pub fn is_setup(&self) -> bool {
        matches!(self.state, CircuitState::Setup(_))
    }

    /// True while the called party is ringing.
    pub fn is_alerting(&self) -> bool {
        matches!(self.state, CircuitState::Alerting(_))
    }

    /// True while the circuit carries traffic, i.e. someone holds the floor on air.
    pub fn is_tx(&self) -> bool {
        matches!(self.state, CircuitState::Tx(_))
    }

    /// True while the circuit is in hangtime after a transmission ceased.
    pub fn is_tx_ceased(&self) -> bool {
        matches!(self.state, CircuitState::TxCeased(_))
    }

    /// True once teardown has started. The circuit stays in the store until the deferred
    /// D-RELEASE has transmitted.
    pub fn is_releasing(&self) -> bool {
        matches!(self.state, CircuitState::Releasing(_))
    }

    /// True before the call is through-connected, so signalling still goes over the control
    /// channel rather than being stolen onto the traffic channel.
    pub fn is_pre_traffic(&self) -> bool {
        self.is_setup() || self.is_alerting()
    }

    // pub fn has_local_ul(&self) -> bool {
    //     match self.ul1_source {
    //         CircuitStreamSrc::Local(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn has_local_dl(&self) -> bool {
    //     match self.dl1_source {
    //         CircuitStreamDest::Local(_) => true,
    //         CircuitStreamDest::LocalAndRemote(_, _) => true,
    //         _ => false,
    //     }
    // }

    // pub fn has_remote_ul(&self) -> bool {
    //     match self.ul1_source {
    //         CircuitStreamSrc::Remote(_) => true,
    //         _ => false,
    //     }
    // }

    // pub fn has_remote_dl(&self) -> bool {
    //     match self.dl1_source {
    //         CircuitStreamDest::Remote(_) => true,
    //         CircuitStreamDest::LocalAndRemote(_, _) => true,
    //         _ => false,
    //     }
    // }

    // fn get_floor(&self) -> Option<u32> {
    //     assert!(!self.is_duplex(), "duplex calls do not have a floor");
    //     return self.floor;
    // }
    // fn grant_floor(&mut self, issi: u32) {
    //     assert!(!self.is_duplex(), "duplex calls do not have a floor");
    //     self.floor = Some(issi);
    // }
    // fn fsm_transition(&mut self, _new_state: CircuitState) {
    //     unimplemented!()
    // }
}

// TODO FIXME below define should be made dynamic once we have multiple carriers
/// Number of carrier frequencies we're using.
pub const NUM_CARRIERS: usize = 1;
/// Number of timeslots we have. 4 per carrier, so increase to 8 when we have a secondary carrier.
pub const NUM_TIMESLOTS: usize = 4 * NUM_CARRIERS;

#[derive(Debug, Clone, Default)]
pub struct CircuitMap {
    pub dl: [Option<CallId>; NUM_TIMESLOTS + 1],
    pub ul: [Option<CallId>; NUM_TIMESLOTS + 1],
}

#[derive(Debug, Clone, Default)]
pub struct CircuitStore {
    circuits: HashMap<CallId, TetraCircuit>,
    map: CircuitMap,
    pub allocator: TimeslotAllocator,
}

impl CircuitStore {
    // TODO: maybe AllocateFreeCircuit(ul, dl) -> &TetraCircuit
    // TODO: get_circuit_for_dl(ts) -> Option<&TetraCircuit>
    // TODO: get_circuit_for_ul(ts) -> Option<&TetraCircuit>
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a struct describing which call IDs are related to which ul and dl timeslots
    fn build_timeslot_map(&self) -> CircuitMap {
        let mut map = CircuitMap::default();

        for (call_id, circuit) in &self.circuits {
            assert!(
                *call_id == circuit.call_id,
                "hashmap key {} doesnt match circuit call_id {}",
                call_id,
                circuit.call_id
            );

            // Get associated dl ts (if any) and store its call id in the map
            let dl = match circuit.dl1_source {
                CircuitStreamDest::Local(ts) => Some(ts),
                CircuitStreamDest::LocalAndRemote(ts, _) => Some(ts),
                _ => None,
            };
            if let Some(ts) = dl {
                let ts = ts.unwrap(); // TetraCircuits in the CircuitStore must be tied to timeslots
                assert!(map.dl[ts as usize].is_none(), "dl ts {} used twice", ts);
                map.dl[ts as usize] = Some(circuit.call_id);
            }

            // Get associated ul ts (if any) and store its call id in the map
            let ul = match circuit.ul1_source {
                CircuitStreamSrc::Local(ts) => Some(ts),
                _ => None,
            };
            if let Some(ts) = ul {
                let ts = ts.unwrap(); // TetraCircuits in the CircuitStore must be tied to timeslots
                assert!(map.ul[ts as usize].is_none(), "ul ts {} used twice", ts);
                map.ul[ts as usize] = Some(circuit.call_id);
            }
        }

        map
    }

    /// Updates the cached timeslot map, computed from the circuits
    fn update_timeslot_map(&mut self) {
        self.map = self.build_timeslot_map()
    }

    /// Retrieves a copy of the timeslot map
    pub fn get_timeslot_map(&self) -> CircuitMap {
        self.map.clone()
    }

    pub fn is_circuit_on_dl_ts(&self, ts: u8) -> bool {
        self.get_callid_by_dl_ts(ts).is_some()
    }

    pub fn get_callid_by_dl_ts(&self, ts: u8) -> Option<CallId> {
        self.map.dl[ts as usize]
    }

    pub fn get_callid_by_ul_ts(&self, ts: u8) -> Option<CallId> {
        self.map.ul[ts as usize]
    }

    /// First circuit matching the predicate. Iteration order is unspecified, so the predicate
    /// should identify at most one circuit.
    pub fn find_circuit<F>(&self, pred: F) -> Option<&TetraCircuit>
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.circuits.values().find(|c| pred(c))
    }

    /// True if any circuit matches the predicate.
    pub fn any_circuit<F>(&self, pred: F) -> bool
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.circuits.values().any(|c| pred(c))
    }

    /// Call ids of every circuit matching the predicate.
    pub fn find_call_ids<F>(&self, pred: F) -> Vec<CallId>
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.circuits.values().filter(|c| pred(c)).map(|c| c.call_id).collect()
    }

    pub fn get_circuit_by_callid(&self, call_id: CallId) -> Option<&TetraCircuit> {
        self.circuits.get(&call_id)
    }

    fn get_circuit_by_callid_mut(&mut self, call_id: CallId) -> Option<&mut TetraCircuit> {
        self.circuits.get_mut(&call_id)
    }

    /// Mutates an existing circuit in place, then refreshes the cached timeslot map.
    /// Returns false if the call_id is not known.
    pub fn update_circuit_with<F>(&mut self, call_id: CallId, f: F) -> bool
    where
        F: FnOnce(&mut TetraCircuit),
    {
        let Some(circuit) = self.get_circuit_by_callid_mut(call_id) else {
            return false;
        };
        f(circuit);
        self.update_timeslot_map();
        true
    }

    /// Sets the state of an existing circuit and stamps the transition time, which restarts the
    /// setup, hangtime or release timer. Returns false if the call_id is not known.
    pub fn set_circuit_state(&mut self, call_id: CallId, state: CircuitState, now: TdmaTime) -> bool {
        self.update_circuit_with(call_id, |c| {
            c.state = state;
            c.t_state = now;
        })
    }

    /// Sets the floor holder of an existing circuit. Returns false if the call_id is not known.
    pub fn set_circuit_floor(&mut self, call_id: CallId, floor: Option<u32>) -> bool {
        self.update_circuit_with(call_id, |c| c.floor = floor)
    }

    // pub fn update_circuit(&mut self, call_id: CallId) {
    //     self.get_circuit_by_callid_mut(call_id);

    //     let do_update = false;

    //     // TODO implement the stuff we need to be able to update.

    //     if do_update {
    //         self.update_timeslot_map();
    //     }
    // }

    pub fn get_circuit_by_dl_ts(&self, ts: u8) -> Option<&TetraCircuit> {
        let call_id = self.get_callid_by_dl_ts(ts)?;
        self.get_circuit_by_callid(call_id)
    }

    pub fn get_circuits(&self) -> &HashMap<CallId, TetraCircuit> {
        &self.circuits
    }

    /// Take a circuit from the state, effectively dropping it from the known circuits
    /// May be used to alter and later re-add the circuit
    /// Returns None if call_id not found
    pub fn take_circuit(&mut self, call_id: CallId) -> Option<TetraCircuit> {
        let ret = self.circuits.remove(&call_id);
        self.update_timeslot_map();
        ret
    }

    pub fn put_circuit(&mut self, circuit: TetraCircuit) {
        let call_id = circuit.call_id;

        // Sanity check on unique call_id
        assert!(
            !self.circuits.contains_key(&call_id),
            "call_id {} already in circuits hashmap",
            call_id
        );

        // Sanity check on circuit timeslot allocations
        if circuit.dl1_source.has_local() {
            assert!(circuit.dl1_source.get_ts().is_some(), "dl1_source local but ts not set");
        }
        if circuit.ul1_source.is_local() {
            assert!(circuit.ul1_source.get_ts().is_some(), "ul1_source local but ts not set");
        }
        if let Some(dl2) = &circuit.dl2_source
            && dl2.has_local()
        {
            assert!(dl2.get_ts().is_some(), "dl2_source local but ts not set");
        }
        if let Some(ul2) = &circuit.ul2_source
            && ul2.is_local()
        {
            assert!(ul2.get_ts().is_some(), "ul2_source local but ts not set");
        }

        self.circuits.insert(call_id, circuit);
        self.update_timeslot_map();
    }

    /// Destroys an existing call. Panics if call_id doesnt exist
    pub fn destroy_circuit_by_callid(&mut self, call_id: CallId) -> TetraCircuit {
        let ret = self.circuits.remove(&call_id);
        assert!(ret.is_some(), "circuit for call_id {} not found", call_id);
        self.update_timeslot_map();
        ret.unwrap() // Never fails after assertion was checked
    }
}
