use tetra_core::TetraAddress;

/// Uplink SDS indication from CMCE to Brew.
///
/// Payload is the SDS Type-4 user data bits, represented as padded bytes.
/// The first octet is typically the SDS Protocol Identifier (PID) as per ETSI.
#[derive(Debug, Clone)]
pub struct BrewSdsRxInd {
    pub source: TetraAddress,
    /// Called party SSI (may be ISSI or GSSI; the stack may not always know which).
    pub destination: TetraAddress,
    /// SDS payload bytes (padded in the last byte if bit_length is not a multiple of 8).
    pub payload: Vec<u8>,
    /// Payload length in bits.
    pub bit_length: u16,
}
