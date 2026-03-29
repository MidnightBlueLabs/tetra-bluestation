// ---------------------------------------------------------------------------
// TelemetryEvent — concrete enum sent through the channel
//
// Small, hot-path variants are inline (no heap allocation).
// Rare / large variants use heap-allocated payload so the enum stays small.
// ---------------------------------------------------------------------------

use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// TelemetryEvent enum sent by a TetraEntity through the TelemetrySink
/// then, serializable by any codec for transmission over the network,
/// using any Transport.
#[derive(Debug, Clone, Encode, Decode, Serialize, Deserialize)]
pub enum TelemetryEvent {
    /// Registration event
    MsRegistration {
        issi: u32,
    },
    /// Deregistration event. Also counts as a deregistration for all groups the ISSI was attached to.
    MsDeregistration {
        issi: u32,
    },
    MsGroupAttach {
        issi: u32,
        gssis: Vec<u32>,
    },
    MsGroupDetach {
        issi: u32,
        gssis: Vec<u32>,
    },
}
