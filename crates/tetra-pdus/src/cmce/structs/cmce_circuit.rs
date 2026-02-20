use tetra_core::{Direction, TdmaTime};
use tetra_saps::{
    control::enums::{circuit_mode_type::CircuitModeType, communication_type::CommunicationType},
    lcmc::CallId,
};

// #[derive(Debug, Clone, Copy, PartialEq)]
// pub struct CmceCircuit {
//     pub direction: Direction,
//     pub call_id: CallId,
//     pub ts: u8,
//     pub endpoint_id: EndpointId,
//     pub circuit_mode_type: CircuitModeType,
//     pub communication_type: CommunicationType,
//     pub simplex_duplex: bool,
//     pub encryption_flag: bool,
//     // pub slots_per_frame: Option<u8>, // only relevant for circuit data
//     /// 2 opt, 00 = TETRA encoded speech, 1|2 = reserved, 3 = proprietary
//     pub speech_service: Option<u8>,
// }

#[derive(Debug, Clone)]
pub struct CmceCircuit {
    /// Time when this circuit was created
    /// Used to schedule D-SETUP repetitions
    pub ts_created: TdmaTime,

    /// Direction
    pub direction: Direction,

    /// Timeslot in which this circuit exists
    pub ts: u8,

    /// Call ID as allocated by CMCE
    pub call_id: CallId,

    /// Usage number, between 4 and 63
    pub usage: u8,

    /// Traffic channel type
    pub circuit_mode: CircuitModeType,

    // pub endpoint_id: EndpointId,
    pub comm_type: CommunicationType,

    pub simplex_duplex: bool,

    // pub slots_per_frame: Option<u8>, // only relevant for circuit data
    /// 2 opt, 00 = TETRA encoded speech, 1|2 = reserved, 3 = proprietary
    pub speech_service: Option<u8>,
    /// Whether end-to-end encryption is enabled on this circuit
    pub etee_encrypted: bool,
}

// impl CmceCircuit {
//     pub fn from_u_setup(
//         direction: Direction,
//         ts: u8,
//         call_id: CallId,
//         usage: u8,
//         pdu: &USetup,
//     ) -> Self {
//         Self {
//             direction,
//             ts,
//             call_id,
//             usage,
//             // endpoint_id: 0,
//             circuit_mode: pdu.basic_service_information.circuit_mode_type,
//             comm_type: pdu.basic_service_information.communication_type,
//             simplex_duplex: true,
//             speech_service: pdu.basic_service_information.speech_service,
//             etee_encrypted: pdu.basic_service_information.encryption_flag,
//         }
//     }
// }
