use tetra_core::Direction;

use crate::control::enums::circuit_mode_type::CircuitModeType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitDlMediaSource {
    /// Downlink media comes from local uplink loopback (classic on-cell behaviour).
    LocalLoopback,
    /// Downlink media is supplied by the network over the Brew bridge.
    Network,
}

#[derive(Debug, Clone)]
pub struct Circuit {
    /// Direction
    pub direction: Direction,

    /// Timeslot in which this circuit exists
    pub ts: u8,

    /// Duplex peer timeslot. When set, uplink voice on this circuit's timeslot is
    /// looped to the downlink of this peer timeslot instead of its own. The two
    /// parties of a duplex call each sit on their own slot and hear the other.
    pub peer_ts: Option<u8>,

    /// Usage number, between 4 and 63
    pub usage: u8,

    /// Traffic channel type
    pub circuit_mode: CircuitModeType,

    // pub comm_type: CommunicationType,

    // pub simplex_duplex: bool,

    // pub slots_per_frame: Option<u8>, // only relevant for circuit data
    /// 2 opt, 00 = TETRA encoded speech, 1|2 = reserved, 3 = proprietary
    pub speech_service: Option<u8>,
    /// Whether end-to-end encryption is enabled on this circuit
    pub etee_encrypted: bool,
    /// Where the downlink audio for this circuit comes from. Local calls loop the
    /// uplink back; network (Brew) calls render audio fed from the backend, so the
    /// local loopback is suppressed.
    pub dl_media_source: CircuitDlMediaSource,
}

/// An individual/circuit call routed over Brew (off-cell ISSI or PBX/phone number).
/// Mirror of the BrewCircularCall wire struct.
#[derive(Debug, Clone)]
pub struct NetworkCircuitCall {
    /// Calling party ISSI
    pub source_issi: u32,
    /// Called party ISSI when known (0 for a number-dialed call)
    pub destination: u32,
    /// External number for PBX/phone calls (ASCII, may be empty)
    pub number: String,
    /// Call priority
    pub priority: u8,
    /// Speech service (Table 14.79)
    pub service: u8,
    /// Circuit mode (Table 14.52)
    pub mode: u8,
    /// Duplex flag (0 = simplex, 1 = duplex)
    pub duplex: u8,
    /// Hook method (Table 14.62)
    pub method: u8,
    /// Communication type (Table 14.54)
    pub communication: u8,
    /// Transmission grant (Table 14.80)
    pub grant: u8,
    /// Transmission request permission (Table 14.81)
    pub permission: u8,
    /// Call timeout (Table 14.50)
    pub timeout: u8,
    /// Call ownership (Table 14.38)
    pub ownership: u8,
    /// Call queued (Table 14.48)
    pub queued: u8,
}

#[derive(Debug, Clone)]
pub enum CallControl {
    /// Signals to set up a circuit
    /// Created by CMCE, sent to Umac
    /// Umac forwards to Lmac
    Open(Circuit),
    /// Signals to release a circuit
    /// Created by CMCE, sent to Umac
    /// Umac forwards to Lmac
    /// Contains (Direction, timeslot) of associated circuit
    Close(Direction, u8),
    /// Floor granted: a speaker has been given transmission permission.
    /// Sent to UMAC to exit hangtime (resume traffic mode) and to Brew to start forwarding voice.
    FloorGranted {
        call_id: u16,
        source_issi: u32,
        dest_gssi: u32,
        ts: u8,
    },
    /// Remote (network/Brew) speaker granted. Sent to UMAC to exit hangtime without arming
    /// the local stuck-uplink detection, since the uplink is silent on a network call.
    RemoteFloorGranted { call_id: u16, ts: u8 },
    /// Floor released: speaker stopped transmitting (entering hangtime).
    /// Sent to UMAC to enter hangtime signalling mode and to Brew to stop forwarding audio.
    FloorReleased { call_id: u16, ts: u8 },
    /// Call ended: the call is being torn down.
    /// Sent to UMAC to clear hangtime state and to Brew to clean up call tracking.
    CallEnded { call_id: u16, ts: u8 },
    /// Request CMCE to start a network-initiated group call
    /// Sent by Brew when TetraPack sends GROUP_TX
    NetworkCallStart {
        brew_uuid: uuid::Uuid, // Brew session UUID for tracking
        source_issi: u32,      // Current speaker
        dest_gssi: u32,        // Target group
        priority: u8,          // Call priority
    },
    /// Notify Brew that network call is ready with allocated resources
    /// Response from CMCE after circuit allocation
    NetworkCallReady {
        brew_uuid: uuid::Uuid, // Matches request
        call_id: u16,          // CMCE-allocated call identifier
        ts: u8,                // Allocated timeslot
        usage: u8,             // Usage number
    },
    /// Request ending a network call
    /// Sent by Brew when TetraPack sends GROUP_IDLE, or by CMCE to make Brew drop a call
    NetworkCallEnd {
        brew_uuid: uuid::Uuid, // Identifies the call to end
    },
    /// UL inactivity detected on a traffic timeslot: no voice frames received
    /// for the timeout period. Sent by UMAC to CMCE.
    UlInactivityTimeout { ts: u8 },
    /// Circuit-call setup request over Brew (individual/PBX/phone), CMCE to Brew.
    NetworkCircuitSetupRequest { brew_uuid: uuid::Uuid, call: NetworkCircuitCall },
    /// Circuit-call setup accepted by the backend, Brew to CMCE.
    NetworkCircuitSetupAccept { brew_uuid: uuid::Uuid },
    /// Circuit-call setup rejected by the backend, Brew to CMCE.
    NetworkCircuitSetupReject { brew_uuid: uuid::Uuid, cause: u8 },
    /// Circuit-call alerting (ringing) from the backend, Brew to CMCE.
    NetworkCircuitAlert { brew_uuid: uuid::Uuid },
    /// Circuit-call connect request from the backend, Brew to CMCE.
    NetworkCircuitConnectRequest { brew_uuid: uuid::Uuid, call: NetworkCircuitCall },
    /// Circuit-call connect confirm from the local side, CMCE to Brew.
    NetworkCircuitConnectConfirm { brew_uuid: uuid::Uuid, grant: u8, permission: u8 },
    /// Circuit-call simplex floor grant.
    NetworkCircuitSimplexGranted { brew_uuid: uuid::Uuid, grant: u8, permission: u8 },
    /// Circuit-call simplex floor idle/release.
    NetworkCircuitSimplexIdle { brew_uuid: uuid::Uuid, grant: u8, permission: u8 },
    /// Circuit-call media is active on this local timeslot, CMCE to Brew.
    NetworkCircuitMediaReady { brew_uuid: uuid::Uuid, call_id: u16, ts: u8 },
    /// Circuit-call DTMF payload from the MS toward the backend.
    NetworkCircuitDtmf {
        brew_uuid: uuid::Uuid,
        length_bits: u16,
        data: Vec<u8>,
    },
    /// Circuit-call release, either direction.
    NetworkCircuitRelease { brew_uuid: uuid::Uuid, cause: u8 },
}
