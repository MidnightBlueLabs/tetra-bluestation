use tetra_core::{BitBuffer, EndpointId, TetraAddress, Todo};

use crate::lcmc::fields::chan_alloc_req::CmceChanAllocReq;

/// Clause 20.4.1.1.1
/// TMA-CANCEL request: this primitive shall be used to cancel a TMA-UNITDATA
/// request primitive that was submitted by the LLC.
#[derive(Debug)]
pub struct TmaCancelReq {
    pub req_handle: Todo,
}

/// Clause 20.4.1.1.2
/// TMA-RELEASE indication: this primitive may be used when the MAC leaves a
/// channel in order to indicate that the connection on that channel is lost
/// (e.g. to indicate local disconnection of any advanced links on that channel).
#[derive(Debug)]
pub struct TmaReleaseInd {
    pub endpoint_id: EndpointId,
}

/// Clause 22.3.3.1.1 gives some hints on reports in the MS context
#[derive(Debug)]
pub enum TmaReport {
    /// Confirm handle to the request
    ConfirmHandle,
    /// MS only. Successful complete transmission by random access
    SuccessRandomAccess,
    /// MS only. Complete transmission by reserved access or stealing
    SuccessReservedOrStealing,

    FailedTransfer,
    FragmentationFailure,
    /// MS only
    RandomAccessFailure,

    /// BS only
    SuccessDownlinked,
}

/// Clause 20.4.1.1.3
/// TMA-REPORT indication: this primitive shall be used by the MAC to report
/// on the progress or failure of a request procedure. The result of the
/// transfer shall be passed as a report parameter.
#[derive(Debug)]
pub struct TmaReportInd {
    pub req_handle: Todo,
    pub report: TmaReport,
}

/// Clause 20.4.1.1.4
/// TMA-UNITDATA request: this primitive shall be used to request the MAC to
/// transmit a TM-SDU.
#[derive(Debug)]
pub struct TmaUnitdataReq {
    pub req_handle: Todo,
    pub pdu: BitBuffer,
    pub main_address: TetraAddress,
    // pub scrambling_code: u32, // TODO FIXME : according to the spec, should be there, but why do we need to provide this?
    pub endpoint_id: EndpointId,
    // pub pdu_prio: Todo, // optional feature
    pub stealing_permission: bool,
    pub subscriber_class: Todo,
    pub air_interface_encryption: Option<Todo>,
    pub stealing_repeats_flag: Option<bool>,
    pub data_category: Option<Todo>,

    // Custom fields for BS stack:
    /// Optional Channel Allocation Request that may be included by CMCE
    pub chan_alloc: Option<CmceChanAllocReq>,
    // Number of identical retransmissions
    // pub redundant_transmission: u8,
}

/// Clause 20.4.1.1.4
/// TMA-UNITDATA indication: this primitive shall be used by the MAC to deliver
/// a received TM-SDU. This primitive may also be used with no TM-SDU if the
/// MAC needs to inform the higher layers of a channel allocation received
/// without an associated TM-SDU.
#[derive(Debug)]
pub struct TmaUnitdataInd {
    pub pdu: Option<BitBuffer>,
    pub main_address: TetraAddress,
    pub scrambling_code: u32,
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: Option<EndpointId>,
    pub css_endpoint_id: Option<EndpointId>,
    pub air_interface_encryption: Todo,
    pub chan_change_response_req: bool,
    pub chan_change_handle: Option<Todo>,
    pub chan_info: Option<Todo>,
}
