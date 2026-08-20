use std::usize;

use tetra_core::TdmaTime;

#[derive(Debug, Clone)]
pub enum CircuitState {
    /// Call was just set up, initial D-SETUPs are being sent out. Contains timeslots elapsed since initial setup
    /// Transitions:
    /// -> Tx after 3 D-SETUPs sent TODO IMPLEMENT
    /// -> TxCeased if U-TX CEASED received TODO IMPLEMENT
    /// -> Releasing if U-RELEASE received TODO IMPLEMENT
    Setup(u32),
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

#[derive(Debug, Clone)]
pub enum CallStreamSource {
    /// The data stream originates from local MS. Holds the local uplink timeslot number
    Local(u8),
    /// The data stream originates from a remote MS. Holds the remote Brew uuid
    Remote(uuid::Uuid),
}

#[derive(Debug, Clone)]
pub enum CallStreamDest {
    /// The data stream is destined for local MS(es). Holds the local downlink timeslot number
    Local(u8),
    /// The data stream is destined for remote MS(es). Holds the remote Brew uuid
    Remote(uuid::Uuid),
    /// The data stream is destined for both local and remote MS(es). Holds the local downlink timeslot number and the remote Brew uuid
    /// Only possible for group calls, as individual calls are always either local or remote.
    LocalAndRemote(u8, uuid::Uuid),
}

#[derive(Debug, Clone)]
pub struct TetraCircuit {
    /// Current state of the call. Element contains data associated uniquely with this state, e.g. elapsed time in setup or hangtime in tx ceased.
    pub state: CircuitState,
    /// Source of the downlink stream. Can be local, remote or both, depending on who's subscribed.
    pub dl_source: CallStreamDest,
    /// Source of the local or remote "uplink" stream.
    pub ul_source: CallStreamSource,

    // TODO: circuit data type, speech service type
    /// Set to true for a duplex call, false for an individual call.
    pub is_duplex: bool,

    /// Set to true for an individual call, false for a group call.
    pub is_individual: bool,

    /// Unique call ID
    pub call_id: u16,
    /// MAC layer usage ID, used to tie signalling data to this call
    pub usage_id: u8,

    /// ISSI that currently holds the floor. Always populated unless for duplex calls
    pub floor: Option<u32>,

    /// ISSI that opened the call. Does not equal the one who now has the floor!
    pub caller: u32,
    /// ISSI or GSSI that was called
    pub callee: u32,

    /// Time of original call start
    pub t_start: TdmaTime,
}

impl TetraCircuit {
    fn is_group_call(&self) -> bool {
        !self.is_individual
    }
    fn is_duplex(&self) -> bool {
        self.is_duplex
    }
    fn has_local_ul(&self) -> bool {
        match self.ul_source {
            CallStreamSource::Local(_) => true,
            _ => false,
        }
    }
    fn has_local_dl(&self) -> bool {
        match self.dl_source {
            CallStreamDest::Local(_) => true,
            CallStreamDest::LocalAndRemote(_, _) => true,
            _ => false,
        }
    }
    fn has_remote_ul(&self) -> bool {
        match self.ul_source {
            CallStreamSource::Remote(_) => true,
            _ => false,
        }
    }
    fn has_remote_dl(&self) -> bool {
        match self.dl_source {
            CallStreamDest::Remote(_) => true,
            CallStreamDest::LocalAndRemote(_, _) => true,
            _ => false,
        }
    }
    fn get_caller(&self) -> u32 {
        return self.caller;
    }
    fn get_callee(&self) -> u32 {
        return self.callee;
    }
    fn get_floor(&self) -> Option<u32> {
        assert!(!self.is_duplex, "duplex calls do not have a floor");
        return self.floor;
    }
    fn grant_floor(&mut self, issi: u32) {
        assert!(!self.is_duplex, "duplex calls do not have a floor");
        self.floor = Some(issi);
    }
    fn get_t_start(&self) -> TdmaTime {
        self.t_start
    }
    fn fsm_transition(&mut self, _new_state: CircuitState) {
        unimplemented!()
    }
}

// TODO FIXME below define should be made dynamic once we have multiple carriers
/// Number of carrier frequencies we're using.
pub const NUM_CARRIERS: usize = 1;
/// Number of timeslots we have. 4 per carrier, so increase to 8 when we have a secondary carrier.
pub const NUM_TIMESLOTS: usize = 4 * NUM_CARRIERS;

#[derive(Debug, Clone)]
pub struct CircuitStore {
    pub circuits: [Option<TetraCircuit>; NUM_TIMESLOTS],
}

impl CircuitStore {
    // TODO: maybe AllocateFreeCircuit(ul, dl) -> &TetraCircuit
    // TODO: get_circuit_for_dl(ts) -> Option<&TetraCircuit>
    // TODO: get_circuit_for_ul(ts) -> Option<&TetraCircuit>
    pub fn new() -> Self {
        Self {
            circuits: [None, None, None, None],
        }
    }

    pub fn get_circuit_by_ts(&self, ts: u8) -> Option<&TetraCircuit> {
        // Unwrap always succeeds if ts < NUM_TIMESLOTS
        self.circuits.get(ts as usize).unwrap().as_ref()
    }

    pub fn is_circuit(&self, ts: u8) -> bool {
        self.circuits[ts as usize].is_some()
    }

    pub fn take_circuit(&mut self, ts: u8) -> Option<TetraCircuit> {
        self.circuits.get_mut(ts as usize).and_then(|slot| slot.take())
    }

    pub fn put_circuit(&mut self, ts: u8, circuit: TetraCircuit) {
        assert!(self.is_circuit(ts));
        self.circuits[ts as usize] = Some(circuit)
    }
}

impl Default for CircuitStore {
    fn default() -> Self {
        Self {
            circuits: [None, None, None, None],
        }
    }
}
