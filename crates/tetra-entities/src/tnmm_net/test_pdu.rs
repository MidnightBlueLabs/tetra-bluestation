use bitcode::{Decode, Encode};

/// Service identifier for Test service: 0x54455354  = ASCII "TEST"
/// Used to detect accidental cross-service message routing
pub const TEST_SERVICE_ID: u32 = 0x54455354;

/// Current protocol version for TestPdu messages
/// Increment when making breaking changes to the protocol
pub const TEST_PDU_VERSION: u32 = 1;

/// Heartbeat tick sent by client to check connection
#[derive(Encode, Decode, Debug)]
pub struct HeartbeatTick {
    pub handle: u32,
}

/// Heartbeat response from server
#[derive(Encode, Decode, Debug)]
pub struct HeartbeatTock {
    pub handle: u32,
}

/// Test request sent by BS to Test Service
#[derive(Encode, Decode, Debug)]
pub struct TestRequest {
    /// Caller-provided handle, echoed in response for correlation
    pub handle: u32,
    /// Subscriber identity (SSI)
    pub ssi: u32,
}

/// Test response from Test Service to BS
#[derive(Encode, Decode, Debug)]
pub struct TestResponse {
    /// Reference to handle in the calling TestRequest
    pub handle: u32,
    /// Subscriber identity (SSI)
    pub ssi: u32,
    /// Test data (unused)
    pub data: u32,
}

/// Envelope message that wraps all Test Service PDU types
/// This allows type-safe deserialization over the wire
#[derive(Encode, Decode, Debug)]
pub struct TnmmTestPduEnvelope {
    /// Service identifier - MUST be TEST_SERVICE_ID (0x54455354 = "TEST")
    /// Used to detect accidental cross-service message routing
    pub service_id: u32,
    /// Protocol version - receiver should check compatibility
    /// Increment when making breaking changes to the protocol
    pub version: u32,
    pub payload: Option<TestPdu>,
}

/// Nested message and enum types in `TestPdu`.
#[derive(Encode, Decode, Debug)]
pub enum TestPdu {
    HeartbeatTick(HeartbeatTick),
    HeartbeatTock(HeartbeatTock),
    TestRequest(TestRequest),
    TestResponse(TestResponse),
}

/// Helper to create a TestPdu with the correct service ID and version
pub fn pack_test_pdu(payload: TestPdu) -> TnmmTestPduEnvelope {
    TnmmTestPduEnvelope {
        service_id: TEST_SERVICE_ID,
        version: TEST_PDU_VERSION,
        payload: Some(payload),
    }
}
