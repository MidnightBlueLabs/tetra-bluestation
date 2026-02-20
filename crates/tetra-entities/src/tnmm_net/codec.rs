use bitcode::{Decode, Encode};

use crate::{network::transports::NetworkError, tnmm_net::test_pdu::TnmmTestPduEnvelope};

#[derive(Encode, Decode)]
struct CompactMessage {
    flags: u8,
    sequence: u32,
    data: Vec<u8>,
}

/// Codec for Test networked service PDU types using bitcode for serialization
#[derive(Default)]
pub struct TnmmTestCodec;

impl TnmmTestCodec {
    /// Encode an TnmmTestPduEnvelope to bitcode
    pub fn encode(&self, pdu: &TnmmTestPduEnvelope) -> Result<Vec<u8>, NetworkError> {
        Ok(bitcode::encode(pdu))
    }

    /// Decode bitcode to an TnmmTestPduEnvelope
    pub fn decode(&self, payload: &[u8]) -> Result<TnmmTestPduEnvelope, NetworkError> {
        bitcode::decode(&payload).map_err(|e| NetworkError::SerializationError(format!("Failed to decode TestPdu: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tnmm_net::test_pdu::*;

    #[test]
    fn test_roundtrip_test_request() {
        let codec = TnmmTestCodec;
        let original = pack_test_pdu(TestPdu::TestRequest(TestRequest { handle: 42, ssi: 12345 }));

        let encoded = codec.encode(&original).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.service_id, TEST_SERVICE_ID);
        assert_eq!(decoded.version, TEST_PDU_VERSION);
        match decoded.payload {
            Some(TestPdu::TestRequest(req)) => {
                assert_eq!(req.handle, 42);
                assert_eq!(req.ssi, 12345);
            }
            _ => panic!("Wrong PDU type decoded"),
        }
    }

    #[test]
    fn test_roundtrip_test_response() {
        let codec = TnmmTestCodec;
        let original = pack_test_pdu(TestPdu::TestResponse(TestResponse {
            handle: 1,
            ssi: 999,
            data: 0xDEADBEEF,
        }));

        let encoded = codec.encode(&original).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.service_id, TEST_SERVICE_ID);
        assert_eq!(decoded.version, TEST_PDU_VERSION);
        match decoded.payload {
            Some(TestPdu::TestResponse(resp)) => {
                assert_eq!(resp.handle, 1);
                assert_eq!(resp.ssi, 999);
                assert_eq!(resp.data, 0xDEADBEEF);
            }
            _ => panic!("Wrong PDU type decoded"),
        }
    }

    #[test]
    fn test_roundtrip_heartbeat() {
        let codec = TnmmTestCodec;
        let original = pack_test_pdu(TestPdu::HeartbeatTick(HeartbeatTick { handle: 7 }));

        let encoded = codec.encode(&original).unwrap();
        let decoded = codec.decode(&encoded).unwrap();

        assert_eq!(decoded.service_id, TEST_SERVICE_ID);
        assert_eq!(decoded.version, TEST_PDU_VERSION);
        match decoded.payload {
            Some(TestPdu::HeartbeatTick(tick)) => {
                assert_eq!(tick.handle, 7);
            }
            _ => panic!("Wrong PDU type decoded"),
        }
    }
}
