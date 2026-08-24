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
    /// Sent to UMAC to clear hangtime state.
    CallEnded { call_id: u16, ts: u8 },
    /// UL inactivity detected on a traffic timeslot: no voice frames received
    /// for the timeout period. Sent by UMAC to CMCE.
    UlInactivityTimeout { ts: u8 },
}
