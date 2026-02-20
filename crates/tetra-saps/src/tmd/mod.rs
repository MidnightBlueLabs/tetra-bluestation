/// Pass TMD circuit data to UMAC for TX scheduling
#[derive(Debug)]
pub struct TmdCircuitDataReq {
    // call_id: CallId,
    pub ts: u8,
    pub data: Vec<u8>,
}

/// Rx'ed traffic
#[derive(Debug)]
pub struct TmdCircuitDataInd {
    // call_id: CallId,
    pub ts: u8,
    pub data: Vec<u8>,
}
