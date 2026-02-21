use tetra_core::Direction;

use crate::control::enums::circuit_mode_type::CircuitModeType;

#[derive(Debug, Clone)]
pub struct Circuit {
    /// Direction
    pub direction: Direction,

    /// Timeslot in which this circuit exists
    pub ts: u8,

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
}

#[derive(Debug)]
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
    /// Floor control: notifies UMAC (and optionally Brew) that the floor was granted to
    /// `source_issi` on `ts` for `dest_gssi`.
    ///
    /// Applies to both locally-originated calls and network-originated (Brew) calls.
    FloorGranted {
        call_id: u16,
        source_issi: u32,
        dest_gssi: u32,
        ts: u8,
    },

    /// Floor control: notifies UMAC (and optionally Brew) that the floor was released
    /// (entering hangtime) for `call_id` on `ts`.
    ///
    /// Applies to both locally-originated calls and network-originated (Brew) calls.
    FloorReleased { call_id: u16, ts: u8 },
    /// Hint from UMAC: likely a rapid PTT re-press (bounce) during hangtime on a traffic timeslot.
    /// Generated when an MS sends MAC-ACCESS on a traffic timeslot that is currently in hangtime.
    /// CMCE may choose to immediately re-grant the floor without waiting for full L3 setup.
    UplinkPttBounce { ts: u8, ssi: u32 },

    /// Notification from UMAC to CMCE: uplink traffic activity detected on a traffic channel.
    ///
    /// Sent when UMAC receives UL TCH (speech/data) while CMCE considers the call in hangtime.
    /// CMCE should treat this as evidence that the MS has re-acquired the floor.
    UplinkTchActivity { ts: u8, ssi: u32 },

    /// Request from CMCE to UMAC: issue a fast downlink slot-grant (MAC layer) for `ssi` on `ts`.
    ///
    /// Used to support rapid re-PTT during hangtime when some terminals only transmit MAC-ACCESS
    /// retries and wait for a MAC grant/ack before they send full L3 signalling or traffic.
    ///
    /// This does NOT change CMCE call state by itself; it is purely a MAC-layer acceleration.
    PttBounceGrant { ts: u8, ssi: u32 },

    /// Call ended: notifies UMAC that the call is fully released and any hangtime state
    /// should be cleared.
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
    /// Request CMCE to end a network call
    /// Sent by Brew when TetraPack sends GROUP_IDLE
    NetworkCallEnd {
        brew_uuid: uuid::Uuid, // Identifies the call to end
    },
}
