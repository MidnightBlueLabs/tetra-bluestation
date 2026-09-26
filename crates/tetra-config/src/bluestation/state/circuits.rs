use std::{collections::HashMap, fmt::Display, usize};

use tetra_core::{SsiType, TdmaTime, TetraAddress, TimeslotAllocator, TxReporter, TxState};
use tetra_pdus::cmce::structs::cmce_circuit::CallId;
use tetra_saps::control::enums::communication_type::CommunicationType;

#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Individual call only: D-SETUP is out and the called party has not answered yet.
    /// Whether it actually reported ringing is tracked by `TetraCircuit::has_alerted`.
    /// Transitions:
    /// -> Tx when the called party answers
    /// -> Releasing on release or no-answer timeout
    Alerting { age: u32 },
    /// Call currently in TX. `age` counts timeslots since the tx segment started; `ul_idle`
    /// counts timeslots since the last local uplink voice frame and is the stuck-talker
    /// timer: UMAC zeroes it through `record_ul_voice`, a floor grant restarts it.
    /// Transitions:
    /// -> TxCeased if U-TX CEASED received
    /// -> Releasing if U-RELEASE received
    Tx { age: u32, ul_idle: u32 },
    /// Call TX ceased, contains timeslots elapsed since hangtime start
    /// Transitions:
    /// -> Tx if Brew receives new data
    /// -> Tx if U TX DEMAND received and granted
    /// -> Releasing if U-RELEASE received
    TxCeased { age: u32 },
    /// Call has been terminated, contains timeslots elapsed since the release started
    Releasing { age: u32 },
}

impl CircuitState {
    /// A fresh TX segment with both counters at zero.
    pub fn new_tx() -> Self {
        CircuitState::Tx { age: 0, ul_idle: 0 }
    }

    /// Timeslots spent in this state, advanced once per tick by `CircuitStore::tick`.
    pub fn get_age(&self) -> u32 {
        match self {
            CircuitState::Alerting { age } | CircuitState::TxCeased { age } | CircuitState::Releasing { age } => *age,
            CircuitState::Tx { age, .. } => *age,
        }
    }

    fn tick(&mut self) {
        match self {
            CircuitState::Alerting { age } | CircuitState::TxCeased { age } | CircuitState::Releasing { age } => *age += 1,
            CircuitState::Tx { age, ul_idle } => {
                *age += 1;
                *ul_idle += 1;
            }
        }
    }
}

/// Number of initial D-SETUP transmissions of a group call, including the very first one.
pub const NUM_SETUP_RETRANSMISSIONS: u32 = 3;
/// After the initial transmissions, a late-entry D-SETUP is repeated this often.
pub const LATE_ENTRY_DSETUP_REPEAT_FRAMES: u32 = 90; // 5 multiframes

/// D-SETUP transmission accounting of a group call. The initial phase sends
/// `NUM_RETRANSMISSIONS` D-SETUPs in subsequent frames, each waiting until the previous one
/// is confirmed sent (or retried if the MAC dropped it). After that, one late-entry D-SETUP
/// goes out every `LATE_ENTRY_DSETUP_REPEAT_FRAMES`.
#[derive(Debug, Clone, Default)]
pub struct SetupRetransmissions {
    /// Receipt of the last D-SETUP handed to the MAC. None once it has been accounted for.
    last: Option<TxReporter>,
    /// Number of D-SETUPs confirmed sent over the air.
    num_sent: u32,
}

impl SetupRetransmissions {
    pub fn record_send(&mut self, receipt: TxReporter) {
        self.last = Some(receipt);
    }

    /// Settle the outstanding receipt: a transmitted D-SETUP counts, a discarded or lost one
    /// is retried without counting. Pending receipts stay put and block the next send.
    pub fn poll(&mut self) {
        let Some(receipt) = &self.last else {
            return;
        };
        match receipt.get_state() {
            TxState::Pending => {}
            TxState::Transmitted | TxState::Acknowledged => {
                self.num_sent += 1;
                self.last = None;
            }
            TxState::Discarded | TxState::Lost => {
                self.last = None;
            }
        }
    }

    /// Whether a D-SETUP send is due, evaluated once per frame.
    pub fn send_due(&self, frames_since_start: u32) -> bool {
        if self.last.is_some() {
            return false; // Previous send still in flight.
        }
        if self.num_sent < NUM_SETUP_RETRANSMISSIONS {
            return true;
        }
        frames_since_start % LATE_ENTRY_DSETUP_REPEAT_FRAMES == 0
    }
}

/// A circuit's data originates either from a local or from a remote MS.
/// In the case of
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

/// Parameters of a call being set up over the network bridge, exchanged between the Brew
/// entity and CMCE while the receiver cannot derive them from a circuit yet. Keyed by Brew
/// session uuid, since the signal announcing the call carries nothing but that uuid.
#[derive(Debug, Clone, Default)]
pub struct NetworkCallRequest {
    /// Calling party ISSI
    pub source: u32,
    /// Called party ISSI or GSSI, zero for a number-dialed call
    pub destination: u32,
    /// External subscriber number for a PBX/phone call, empty otherwise
    pub number: String,
    /// Call priority
    pub priority: u8,
    /// Speech service (ETSI Table 14.79)
    pub service: u8,
    /// Circuit mode (ETSI Table 14.52)
    pub mode: u8,
    /// Whether the call is a duplex call; only possible for individual calls
    pub is_duplex: u8,
    /// Hook method (ETSI Table 14.62)
    pub method: u8,
    /// Communication type (ETSI Table 14.54)
    pub communication: u8,
    /// Transmission grant (ETSI Table 14.80)
    pub grant: u8,
    /// Transmission request permission (ETSI Table 14.81)
    pub permission: u8,
    /// Call timeout (ETSI Table 14.50)
    pub timeout: u8,
    /// Call ownership (ETSI Table 14.38)
    pub ownership: u8,
    /// Call queued (ETSI Table 14.48)
    pub queued: u8,
}

#[derive(Debug, Clone)]
pub struct TetraCircuit {
    /// Unique call ID
    pub call_id: u16,

    /// Current state of the call. Carries the number of timeslots spent in this state,
    /// advanced by `CircuitStore::tick` and reset on every state change. Drives the
    /// hangtime, call and release timers.
    pub state: CircuitState,

    /// D-SETUP transmission accounting for a group call. Unused on individual calls, whose
    /// single D-SETUP goes over the acknowledged link.
    pub setup_retrans: SetupRetransmissions,

    /// Individual call: the called party has reported ringing (U-ALERT or backend alert).
    pub has_alerted: bool,

    /// The circuit was in hangtime when its release started, so it keeps that channel mode
    /// until teardown. Only meaningful while `state` is `Releasing`.
    pub hangtime_at_release: bool,

    /// Source of the downlink stream. Can be local, remote or both, depending on who's subscribed.
    pub dl1_source: CircuitStreamDest,
    /// Source of the local or remote "uplink" stream.
    pub ul1_source: CircuitStreamSrc,
    /// MAC layer usage ID, used to tie signalling data to this call
    pub usage_id: u8,

    /// Source of the duplex secondary downlink stream.
    /// Can be local, remote or both, depending on who's subscribed.
    pub dl2_source: Option<CircuitStreamDest>,
    /// Source of the duplex secondary local or remote "uplink" stream.
    pub ul2_source: Option<CircuitStreamSrc>,
    /// Duplex channel MAC layer usage ID
    pub usage2_id: Option<u8>,

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

    /// Only P2p (individual) and P2mp supported now
    pub comm_type: CommunicationType,

    /// ISSI that currently holds the floor. Always populated for half-duplex calls when in Tx
    /// Designates who's currently speaking
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
    pub brew_origin_uuid: Option<uuid::Uuid>,

    /// Call voice frames are E2EE encrypted
    pub is_etee_encrypted: bool,

    /// Time of original call start
    pub t_start: TdmaTime,
}

impl TetraCircuit {
    pub fn is_group_call(&self) -> bool {
        matches!(self.comm_type, CommunicationType::P2MpAcked | CommunicationType::P2mp)
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

    /// Local downlink timeslots this circuit occupies: its own, plus the second channel of a
    /// duplex call.
    pub fn dl_slots(&self) -> [Option<u8>; 2] {
        [self.dl1_source.get_ts(), self.dl2_source.and_then(|d| d.get_ts())]
    }

    /// Local uplink timeslots this circuit occupies. A remote uplink (a network speaker on a
    /// group call) still holds the local slot, so it is taken from the downlink instead.
    pub fn ul_slots(&self) -> [Option<u8>; 2] {
        [
            self.ul1_source.get_ts().or_else(|| self.dl1_source.get_ts()),
            self.ul2_source
                .and_then(|s| s.get_ts())
                .or_else(|| self.dl2_source.and_then(|d| d.get_ts())),
        ]
    }

    /// MAC usage marker to advertise on the given timeslot.
    pub fn usage_for_ts(&self, ts: u8) -> u8 {
        match self.ul2_source.and_then(|s| s.get_ts()) {
            Some(second) if second == ts => self.callee_usage_id(),
            _ => self.usage_id,
        }
    }

    /// Other half of a cross-routed duplex pair: the timeslot whose downlink carries what is
    /// received on this timeslot's uplink. None unless a second channel is allocated.
    pub fn peer_of_ts(&self, ts: u8) -> Option<u8> {
        let first = self.ul1_source.get_ts()?;
        let second = self.ul2_source?.get_ts()?;
        match ts {
            _ if ts == first => Some(second),
            _ if ts == second => Some(first),
            _ => None,
        }
    }

    /// Downlink audio comes from the network rather than from a local uplink, so the MAC must
    /// not loop uplink speech back onto the downlink.
    pub fn dl_is_from_network(&self) -> bool {
        (self.is_individual_call() && self.is_over_brew()) || self.ul1_source.is_remote()
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

    /// True while D-SETUP is out but the called party has not answered. Individual calls only.
    pub fn is_alerting(&self) -> bool {
        matches!(self.state, CircuitState::Alerting { .. })
    }

    /// True while the circuit carries traffic, i.e. someone holds the floor on air.
    pub fn is_tx(&self) -> bool {
        matches!(self.state, CircuitState::Tx { .. })
    }

    /// True while the circuit is in hangtime after a transmission ceased.
    pub fn is_tx_ceased(&self) -> bool {
        matches!(self.state, CircuitState::TxCeased { .. })
    }

    /// True once teardown has started. The circuit stays in the store until the deferred
    /// D-RELEASE has transmitted.
    pub fn is_releasing(&self) -> bool {
        matches!(self.state, CircuitState::Releasing { .. })
    }

    /// The slot stays allocated but carries signalling instead of traffic. A group call marks
    /// hangtime with `TxCeased`; an individual simplex call stays in `Tx` (its call-length
    /// timer keeps running) and marks it by a free floor. Duplex and over-Brew individual
    /// calls have no hangtime: the downlink keeps playing. A releasing circuit keeps the
    /// mode it had when the release started, so the stolen D-RELEASE goes out the same way.
    pub fn in_hangtime(&self) -> bool {
        if self.is_releasing() {
            return self.hangtime_at_release;
        }
        self.is_tx_ceased()
            || (self.is_individual_call() && !self.is_duplex && !self.is_over_brew() && self.is_tx() && self.floor.is_none())
    }

    /// Local uplink voice is expected on this circuit's slot(s): a party on air holds the
    /// floor and the downlink is not fed by the network. Drives the MAC's stuck-talker
    /// detection. False for duplex calls (no floor, so silence is not a stuck talker).
    pub fn expect_local_ul(&self) -> bool {
        self.is_tx() && self.floor.is_some() && !self.dl_is_from_network()
    }
}

impl Display for TetraCircuit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TetraCircuit {{ call_id {} state {:?} caller {} callee {} }}",
            self.call_id, self.state, self.caller, self.callee
        )
    }
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

/// What the MAC needs to know about the circuit occupying a traffic timeslot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacSlot {
    pub call_id: CallId,
    /// Usage marker to advertise for this slot on the AACH.
    pub usage: u8,
    /// Timeslot whose downlink carries this slot's uplink speech, for a cross-routed duplex call.
    pub peer_ts: Option<u8>,
    /// Downlink speech comes from the network, so local uplink must not be looped back.
    pub dl_from_network: bool,
    /// The call is in hangtime: keep the slot allocated but carry signalling, not traffic.
    pub in_hangtime: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CircuitStore {
    circuits: HashMap<CallId, TetraCircuit>,
    circuit_map: CircuitMap,
    uuid_map: HashMap<uuid::Uuid, CallId>,
    network_requests: HashMap<uuid::Uuid, NetworkCallRequest>,
    closing_sessions: HashMap<CallId, uuid::Uuid>,
    pub allocator: TimeslotAllocator,
}

impl CircuitStore {
    // TODO: maybe AllocateFreeCircuit(ul, dl) -> &TetraCircuit
    // TODO: get_circuit_for_dl(ts) -> Option<&TetraCircuit>
    // TODO: get_circuit_for_ul(ts) -> Option<&TetraCircuit>
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a struct describing which call IDs are related to which ul and dl timeslots.
    /// A circuit claims its timeslots for as long as it exists, regardless of where its media
    /// currently comes from, so this doubles as the MAC's view of which slots carry traffic.
    fn build_timeslot_map(&self) -> CircuitMap {
        let mut map = CircuitMap::default();

        for (call_id, circuit) in &self.circuits {
            assert!(
                *call_id == circuit.call_id,
                "hashmap key {} doesnt match circuit call_id {}",
                call_id,
                circuit.call_id
            );

            for ts in circuit.dl_slots().into_iter().flatten() {
                assert!(map.dl[ts as usize].is_none(), "dl ts {} used twice", ts);
                map.dl[ts as usize] = Some(circuit.call_id);
            }

            for ts in circuit.ul_slots().into_iter().flatten() {
                assert!(map.ul[ts as usize].is_none(), "ul ts {} used twice", ts);
                map.ul[ts as usize] = Some(circuit.call_id);
            }
        }

        map
    }

    fn build_uuid_map(&self) -> HashMap<uuid::Uuid, CallId> {
        let mut ret: HashMap<uuid::Uuid, CallId> = HashMap::new();

        // The same Brew session commonly feeds both the uplink and the downlink of one
        // circuit, so a repeat is only a conflict when it points at a different call.
        let mut insert = |uuid: Option<uuid::Uuid>, call_id: CallId| {
            let Some(uuid) = uuid else {
                return;
            };
            let previous = ret.insert(uuid, call_id);
            assert!(
                previous.is_none_or(|prev| prev == call_id),
                "uuid {} used by call_id {} and {}",
                uuid,
                previous.unwrap(),
                call_id
            );
        };

        for (call_id, circuit) in &self.circuits {
            if let CircuitStreamDest::Remote(uuid) | CircuitStreamDest::LocalAndRemote(_, uuid) = circuit.dl1_source {
                insert(uuid, *call_id);
            }

            if let CircuitStreamSrc::Remote(uuid) = circuit.ul1_source {
                insert(uuid, *call_id);
            }

            if let Some(CircuitStreamDest::Remote(uuid) | CircuitStreamDest::LocalAndRemote(_, uuid)) = circuit.dl2_source {
                insert(uuid, *call_id);
            }

            if let Some(CircuitStreamSrc::Remote(uuid)) = circuit.ul2_source {
                insert(uuid, *call_id);
            }
        }
        ret
    }

    /// Updates the cached timeslot map, computed from the circuits
    fn update_maps(&mut self) {
        self.circuit_map = self.build_timeslot_map();
        self.uuid_map = self.build_uuid_map();
        println!("{}", self.dump_maps_verbose());
    }

    /// Renders which call occupies which timeslot, for debugging the derived MAC view.
    pub fn dump_maps(&self) -> String {
        let mut out = String::new();
        for ts in 1..=NUM_TIMESLOTS {
            let dl = self.circuit_map.dl[ts];
            let ul = self.circuit_map.ul[ts];
            if dl.is_none() && ul.is_none() {
                continue;
            }
            out.push_str(&format!("ts {}: dl {:?}, ul {:?}; ", ts, dl, ul));
        }
        out
    }

    pub fn dump_maps_verbose(&self) -> String {
        let mut out = String::new();
        out.push_str("---------------------------- Circuit map ----------------------------\n");
        for ts in 1..=NUM_TIMESLOTS {
            let dl_call_id = self.circuit_map.dl[ts];
            let ul_call_id = self.circuit_map.ul[ts];
            if let Some(cid) = dl_call_id {
                out.push_str(&format!("ts {}: dl {}\n", ts, self.get_circuit_by_callid(cid).unwrap()));
            }
            if let Some(cid) = ul_call_id {
                out.push_str(&format!("ts {}: ul {}\n", ts, self.get_circuit_by_callid(cid).unwrap()));
            }
        }
        out.push_str("---------------------------------------------------------------------\n");
        out
    }

    /// Retrieves a copy of the timeslot map
    pub fn get_timeslot_map(&self) -> CircuitMap {
        self.circuit_map.clone()
    }

    pub fn is_circuit_on_dl_ts(&self, ts: u8) -> bool {
        self.get_callid_by_dl_ts(ts).is_some()
    }

    pub fn get_callid_by_dl_ts(&self, ts: u8) -> Option<CallId> {
        self.circuit_map.dl[ts as usize]
    }

    pub fn get_callid_by_ul_ts(&self, ts: u8) -> Option<CallId> {
        self.circuit_map.ul[ts as usize]
    }

    pub fn get_callid_by_uuid(&self, uuid: uuid::Uuid) -> Option<CallId> {
        self.uuid_map.get(&uuid).copied()
    }

    /// The MAC's view of a traffic timeslot: everything the scheduler needs to build the slot,
    /// derived from the circuit occupying it. None when the timeslot carries no circuit.
    pub fn mac_slot(&self, ts: u8) -> Option<MacSlot> {
        let call_id = self.get_callid_by_dl_ts(ts).or_else(|| self.get_callid_by_ul_ts(ts))?;
        let circuit = self.get_circuit_by_callid(call_id)?;
        Some(MacSlot {
            call_id,
            usage: circuit.usage_for_ts(ts),
            peer_ts: circuit.peer_of_ts(ts),
            dl_from_network: circuit.dl_is_from_network(),
            in_hangtime: circuit.in_hangtime(),
        })
    }

    pub fn get_circuit_by_uuid(&self, uuid: uuid::Uuid) -> Option<&TetraCircuit> {
        let call_id = self.get_callid_by_uuid(uuid)?;
        self.get_circuit_by_callid(call_id)
    }

    /// Deposits the parameters of a call announced over the network bridge, for the entity
    /// handling the accompanying signal to pick up.
    pub fn put_network_request(&mut self, uuid: uuid::Uuid, request: NetworkCallRequest) {
        self.network_requests.insert(uuid, request);
    }

    pub fn has_network_request(&self, uuid: uuid::Uuid) -> bool {
        self.network_requests.contains_key(&uuid)
    }

    /// Collects the deposited parameters of a network call. They are consumed, so a signal is
    /// always accompanied by a fresh deposit.
    pub fn take_network_request(&mut self, uuid: uuid::Uuid) -> Option<NetworkCallRequest> {
        self.network_requests.remove(&uuid)
    }

    /// Hands the network bridge session of a call over to the entity that has to close it
    /// upstream. Deposited by the entity destroying the circuit, so the session outlives the
    /// call it belonged to just long enough to be signalled by call id.
    pub fn put_closing_session(&mut self, call_id: CallId, uuid: uuid::Uuid) {
        self.closing_sessions.insert(call_id, uuid);
    }

    /// Collects the network bridge session left behind by a destroyed call.
    pub fn take_closing_session(&mut self, call_id: CallId) -> Option<uuid::Uuid> {
        self.closing_sessions.remove(&call_id)
    }

    /// First circuit matching the predicate. Iteration order is unspecified, so the predicate
    /// should identify at most one circuit.
    pub fn find_circuit<F>(&self, pred: F) -> Option<&TetraCircuit>
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.circuits.values().find(|c| pred(c))
    }

    /// A live circuit by call id. A circuit in `Releasing` is not live: a call already being
    /// torn down can never be reused or answered.
    pub fn live_circuit(&self, call_id: CallId) -> Option<&TetraCircuit> {
        self.get_circuit_by_callid(call_id).filter(|c| !c.is_releasing())
    }

    /// First live circuit matching the predicate.
    pub fn find_live_circuit<F>(&self, pred: F) -> Option<&TetraCircuit>
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.find_circuit(|c| !c.is_releasing() && pred(c))
    }

    /// True if any live circuit matches the predicate.
    pub fn any_live_circuit<F>(&self, pred: F) -> bool
    where
        F: Fn(&TetraCircuit) -> bool,
    {
        self.any_circuit(|c| !c.is_releasing() && pred(c))
    }

    /// A live individual (point-to-point) call by call id.
    pub fn live_individual_circuit(&self, call_id: CallId) -> Option<&TetraCircuit> {
        self.live_circuit(call_id).filter(|c| c.is_individual_call())
    }

    /// A live group call by call id.
    pub fn live_group_circuit(&self, call_id: CallId) -> Option<&TetraCircuit> {
        self.live_circuit(call_id).filter(|c| c.is_group_call())
    }

    /// A Brew session is known while its call exists, or while its parameters still await
    /// pickup by CMCE (setup in flight).
    pub fn is_known_session(&self, uuid: uuid::Uuid) -> bool {
        self.get_callid_by_uuid(uuid).is_some() || self.has_network_request(uuid)
    }

    /// Detaches a Brew session from its call, leaving the circuit purely local. Used once the
    /// upstream session is over while the call itself may live on (hangtime, new speaker).
    pub fn detach_session(&mut self, uuid: uuid::Uuid) {
        let Some(call_id) = self.get_callid_by_uuid(uuid) else {
            return;
        };
        let Some(ts) = self.get_circuit_by_callid(call_id).map(|circuit| circuit.dl_ts()) else {
            return;
        };
        self.update_circuit_with(call_id, |circuit| circuit.dl1_source = CircuitStreamDest::Local(Some(ts)));
    }

    /// Advances D-SETUP retransmission accounting and returns the group calls due for a
    /// (re-)send as (call id, usage marker, downlink timeslot). Evaluated once per frame.
    /// Hangtime suppresses the sends: the traffic channel is still allocated and a D-SETUP
    /// with NotGranted can prevent floor requests.
    pub fn dsetup_sends_due(&mut self, now: TdmaTime) -> Vec<(CallId, u8, u8)> {
        let mut due = Vec::new();
        for circuit in self.circuits.values_mut() {
            if !circuit.is_group_call() || circuit.is_releasing() {
                continue;
            }
            circuit.setup_retrans.poll();
            if circuit.is_tx_ceased() {
                continue;
            }
            let frames_since_start = (circuit.t_start.age(now) / 4).max(0) as u32;
            if circuit.setup_retrans.send_due(frames_since_start) {
                due.push((circuit.call_id, circuit.usage_id, circuit.dl_ts()));
            }
        }
        due
    }

    /// Records a D-SETUP handed to the MAC, so the next one waits for its receipt.
    pub fn record_dsetup_send(&mut self, call_id: CallId, receipt: TxReporter) {
        if let Some(circuit) = self.circuits.get_mut(&call_id) {
            circuit.setup_retrans.record_send(receipt);
        }
    }

    /// Zeroes the uplink inactivity timer of the transmitting circuit on this uplink
    /// timeslot. Called by UMAC for every uplink voice frame; the only shared-state write
    /// outside CMCE, so it deliberately touches nothing else.
    pub fn umac_update_ul_voice_timer(&mut self, ts: u8) {
        if !(1..=NUM_TIMESLOTS as u8).contains(&ts) {
            return;
        }
        let Some(call_id) = self.get_callid_by_ul_ts(ts) else {
            return;
        };
        if let Some(circuit) = self.circuits.get_mut(&call_id)
            && let CircuitState::Tx { ul_idle, .. } = &mut circuit.state
        {
            *ul_idle = 0;
        }
    }

    /// Call ids whose talker has gone silent: local uplink voice expected but none arrived
    /// for more than `threshold` timeslots. The timer restarts on report, so a call that
    /// somehow survives the resulting cease is reported once per period, not every tick.
    pub fn take_ul_inactive_calls(&mut self, threshold: u32) -> Vec<CallId> {
        let mut out = Vec::new();
        for circuit in self.circuits.values_mut() {
            if !circuit.expect_local_ul() {
                continue;
            }
            if let CircuitState::Tx { ul_idle, .. } = &mut circuit.state
                && *ul_idle > threshold
            {
                *ul_idle = 0;
                out.push(circuit.call_id);
            }
        }
        out
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
        self.update_maps();
        true
    }

    /// Advances every circuit's time-in-state counter by one timeslot. Driven once per tick
    /// by the circuit manager.
    pub fn tick(&mut self) {
        for circuit in self.circuits.values_mut() {
            circuit.state.tick();
        }
    }

    /// Sets the state of an existing circuit, which restarts its time-in-state counter and
    /// thereby the setup, hangtime or release timer. Returns false if the call_id is not known.
    pub fn set_circuit_state(&mut self, call_id: CallId, state: CircuitState) -> bool {
        self.update_circuit_with(call_id, |c| {
            c.state = state;
        })
    }

    /// Sets the floor holder of an existing circuit. Returns false if the call_id is not known.
    /// A grant also restarts the uplink inactivity timer, so the fresh talker does not
    /// inherit the previous one's silence.
    pub fn set_circuit_floor(&mut self, call_id: CallId, floor: Option<u32>) -> bool {
        self.update_circuit_with(call_id, |c| {
            c.floor = floor;
            if floor.is_some()
                && let CircuitState::Tx { ul_idle, .. } = &mut c.state
            {
                *ul_idle = 0;
            }
        })
    }

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
        self.update_maps();
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
        self.update_maps();
    }

    /// Destroys an existing call. Panics if call_id doesnt exist
    pub fn destroy_circuit_by_callid(&mut self, call_id: CallId) -> TetraCircuit {
        let ret = self.circuits.remove(&call_id);
        assert!(ret.is_some(), "circuit for call_id {} not found", call_id);
        self.update_maps();
        ret.unwrap() // Never fails after assertion was checked
    }
}
