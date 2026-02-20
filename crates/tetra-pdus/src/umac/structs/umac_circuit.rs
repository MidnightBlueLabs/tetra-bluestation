// use tetra_core::Todo;
// use tetra_saps::{lcmc::{CallId, control::Circuit, enums::circuit_mode_type::CircuitModeType}, tmv::enums::logical_chans::LogicalChannel};

// pub struct UmacCircuit {
//     /// Timeslot in which this circuit exists
//     pub ts: u8,
//     /// Call ID as allocated by CMCE
//     pub call_id: CallId,
//     /// Traffic channel type
//     pub circuit_mode: CircuitModeType,
//     /// Usage number, between 4 and 63
//     pub usage: u8,
//     pub encryption: Option<Todo>,
// }

// impl UmacCircuit {
//     pub fn from_cmce_circuit(circuit: Circuit) -> Self {
//         assert!(circuit.usage >= 4 && circuit.usage <= 63);
//         assert!(circuit.ts >= 2 && circuit.ts <= 4);
//         assert!(Self::get_logical_channel(circuit.circuit_mode).is_traffic());
//         UmacCircuit {
//             ts: circuit.ts,
//             call_id: circuit.call_id,
//             circuit_mode: circuit.circuit_mode,
//             usage: circuit.usage,
//             encryption: circuit.encryption,
//         }
//     }

//     pub fn get_logical_channel(circuit_mode: CircuitModeType) -> LogicalChannel {
//         match circuit_mode {
//             CircuitModeType::TchS => LogicalChannel::TchS,
//             CircuitModeType::Tch72 => LogicalChannel::Tch72,
//             CircuitModeType::Tch48n1 |
//             CircuitModeType::Tch48n4 |
//             CircuitModeType::Tch48n8 => LogicalChannel::Tch48,
//             CircuitModeType::Tch24n1 |
//             CircuitModeType::Tch24n4 |
//             CircuitModeType::Tch24n8 => LogicalChannel::Tch24,
//         }
//     }
// }
