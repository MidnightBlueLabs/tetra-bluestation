use tetra_core::{BitBuffer, EndpointId, Todo};

/// BS only
/// TL-SAP and TMB-SAP merged into TLMB-SAP
#[derive(Debug)]
pub struct TlmbSyncReq {
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
    pub priority: Todo,
}

/// MS only
/// TL-SAP and TMB-SAP merged into TLMB-SAP
#[derive(Debug)]
pub struct TlmbSyncInd {
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
}

/// BS only
/// TL-SAP and TMB-SAP merged into TLMB-SAP
#[derive(Debug)]
pub struct TlmbSysinfoReq {
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
    pub mac_broadcast_info: Option<Todo>,
    pub priority: Todo,
}

/// MS only
/// TL-SAP and TMB-SAP merged into TLMB-SAP
#[derive(Debug)]
pub struct TlmbSysinfoInd {
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
    pub mac_broadcast_info: Option<Todo>,
}
