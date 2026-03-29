//! Telemetry codec — bitcode-based binary serialization of [`TelemetryEvent`]s.

use crate::{net_telemetry::events::TelemetryEvent, network::transports::NetworkError};

// ---------------------------------------------------------------------------
// Codecs
// ---------------------------------------------------------------------------

/// Codec for telemetry events using bitcode for serialization.
#[derive(Default)]
pub struct TelemetryCodecBitcode;

impl TelemetryCodecBitcode {
    /// Encode a [`TelemetryEvent`] to bitcode bytes.
    pub fn encode(&self, event: &TelemetryEvent) -> Vec<u8> {
        bitcode::encode(event)
    }

    /// Decode bitcode bytes into a [`TelemetryEvent`].
    pub fn decode(&self, payload: &[u8]) -> Result<TelemetryEvent, NetworkError> {
        bitcode::decode(payload).map_err(|e| NetworkError::SerializationError(format!("telemetry decode: {}", e)))
    }
}

/// Codec for telemetry events using JSON for serialization.
#[derive(Default)]
pub struct TelemetryCodecJson;

impl TelemetryCodecJson {
    /// Encode a [`TelemetryEvent`] to JSON bytes.
    pub fn encode(&self, event: &TelemetryEvent) -> Vec<u8> {
        serde_json::to_vec(event).unwrap_or_default()
    }

    /// Decode JSON bytes into a [`TelemetryEvent`].
    pub fn decode(&self, payload: &[u8]) -> Result<TelemetryEvent, NetworkError> {
        serde_json::from_slice(payload).map_err(|e| NetworkError::SerializationError(format!("telemetry decode: {}", e)))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_bitcode_registration() {
        let codec = TelemetryCodecBitcode;
        let event = TelemetryEvent::MsRegistration { issi: 1234 };
        let bytes = codec.encode(&event);
        let decoded = codec.decode(&bytes).unwrap();
        let TelemetryEvent::MsRegistration { issi } = decoded else {
            panic!("expected Registration");
        };
        assert_eq!(issi, 1234);
    }

    #[test]
    fn test_roundtrip_json_registration() {
        let codec = TelemetryCodecJson;
        let event = TelemetryEvent::MsRegistration { issi: 1234 };
        let bytes = codec.encode(&event);
        let decoded = codec.decode(&bytes).unwrap();
        let TelemetryEvent::MsRegistration { issi } = decoded else {
            panic!("expected Registration");
        };
        assert_eq!(issi, 1234);
    }

    #[test]
    fn test_decode_invalid_bytes() {
        let codec = TelemetryCodecBitcode;
        assert!(codec.decode(&[0xFF, 0x00]).is_err());
    }
}
