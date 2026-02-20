#![allow(unused)]
use tetra_core::{BitBuffer, EndpointId, LinkId, TetraAddress, Todo};

use crate::lcmc::fields::chan_alloc_req::CmceChanAllocReq;

#[derive(Debug)]
pub struct TlCancelReq {
    pub handle: Todo,
}

/// advanced link
#[derive(Debug)]
pub struct TlConnectReq {
    // address_type: Todo,
    main_address: Todo,
    scrambling_code: Todo,
    link_id: LinkId,
    endpoint_id: EndpointId,
    pdu_prio: Todo,
    stealing_permission: bool,
    subscriber_class: Todo,
    qos: Todo,
    al_service: Todo,
    air_interface_encryption: Todo,
    req_handle: Todo,
    setup_report: Todo,
}
/// advanced link
#[derive(Debug)]
pub struct TlConnectInd {
    // address_type: Todo,
    main_address: Todo,
    scrambling_code: Todo,
    link_id: LinkId,
    endpoint_id: EndpointId,
    new_endpoint_id: Option<Todo>,
    css_endpoint_id: Option<Todo>,
    qos: Todo,
    al_service: Todo,
    air_interface_encryption: Todo,
    chan_change_resp_req: bool,
    chan_change_handle: Option<Todo>,
    chan_info: Option<Todo>,
    req_handle: Todo,
    setup_report: Todo,
}
/// advanced link
#[derive(Debug)]
pub struct TlConnectResp {
    // address_type: Todo,
    main_address: Todo,
    scrambling_code: Todo,
    link_id: LinkId,
    endpoint_id: EndpointId,
    pdu_prio: Todo,
    stealing_permission: bool,
    subscriber_class: Todo,
    qos: Todo,
    al_service: Todo,
    air_interface_encryption: Todo,
    req_handle: Todo,
    setup_report: Todo,
}
/// advanced link
#[derive(Debug)]
pub struct TlConnectConf {
    // address_type: Todo,
    main_address: Todo,
    scrambling_code: Todo,
    link_id: LinkId,
    endpoint_id: EndpointId,
    new_endpoint_id: Option<Todo>,
    css_endpoint_id: Option<Todo>,
    qos: Todo,
    al_service: Todo,
    air_interface_encryption: Todo,
    chan_change_resp_req: bool,
    chan_change_handle: Option<Todo>,
    chan_info: Option<Todo>,
    req_handle: Todo,
    setup_report: Todo,
}

/// advanced link only
#[derive(Debug)]
pub struct TlDataReqAl;
#[derive(Debug)]
pub struct TlDataIndAl;
#[derive(Debug)]
pub struct TlDataConfAl;

/// Clause 20.3.5.1.4
/// TL-DATA request: this primitive shall be used by the layer 2 service user to request transmission of a TL-SDU. The
// TL-SDU will be acknowledged by the peer entity.
#[derive(Debug)]
pub struct TlaTlDataReqBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
    // pub scrambling_code: u32, // TODO FIXME: according to the spec, should be there, but why do we need to provide this?
    // pub pdu_prio: Todo, // Optional feature
    pub stealing_permission: bool,
    pub subscriber_class: Todo,
    pub fcs_flag: bool,
    pub air_interface_encryption: Option<Todo>,
    pub stealing_repeats_flag: Option<bool>,
    pub data_class_info: Option<Todo>,
    pub req_handle: Todo,
    pub graceful_degradation: Option<Todo>,

    // Custom fields for BS stack:
    /// Optional Channel Allocation Request that may be included by CMCE
    pub chan_alloc: Option<CmceChanAllocReq>,
    // Number of identical retransmissions
    // pub redundant_transmission: u8,
}

/// Clause 20.3.5.1.4
/// TL-DATA indication: this primitive shall be used by the layer 2 to deliver the received TL-SDU to the layer 2 service
// user.
#[derive(Debug)]
pub struct TlaTlDataIndBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: Option<EndpointId>,
    pub css_endpoint_id: Option<EndpointId>,
    pub tl_sdu: Option<BitBuffer>,
    pub scrambling_code: u32,
    pub fcs_flag: bool,
    pub air_interface_encryption: Todo,
    pub chan_change_resp_req: bool,
    pub chan_change_handle: Option<Todo>,
    pub chan_info: Option<Todo>,
    pub req_handle: Todo,
}

/// Clause 20.3.5.1.4
/// TL-DATA response: this primitive shall be used by the layer 2 service user to respond to the previous TL-DATA
// indication primitive. The TL-DATA response primitive may contain a TL-SDU. That TL-SDU will be sent without an
// explicit acknowledgement from the peer entity.
#[derive(Debug)]
pub struct TlDataRespBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
    pub scrambling_code: Todo,
    pub pdu_prio: Todo,
    pub stealing_permission: bool,
    pub subscriber_class: Todo,
    pub fcs_flag: bool,
    pub air_interface_encryption: Todo,
    pub stealing_repeats_flag: Option<bool>,
    pub data_class_info: Option<Todo>,
    pub req_handle: Todo,
}

/// Clause 20.3.5.1.4
// TL-DATA confirm: this primitive shall be used by the layer 2 to inform the layer 2 service user that it has completed
// successfully the transmission of the requested TL-SDU. Depending on the availability of the response primitive at the
// peer entity before transmission of the acknowledgement, the confirm primitive may or may not carry a TL-SDU.
#[derive(Debug)]
pub struct TlDataConfBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: Option<Todo>,
    pub css_endpoint_id: Option<Todo>,
    pub tl_sdu: Option<BitBuffer>,
    pub scrambling_code: Todo,
    pub fcs_flag: bool,
    pub air_interface_encryption: Todo,
    pub chan_change_resp_req: bool,
    pub chan_change_handle: Option<Todo>,
    pub chan_info: Option<Todo>,
    pub req_handle: Todo,
    pub report: Todo,
}

/// Advanced link only
#[derive(Debug)]
pub struct TlDisconnectReq;
/// Advanced link only
#[derive(Debug)]
pub struct TlDisconnectInd;
/// Advanced link only
#[derive(Debug)]
pub struct TlDisconnectConf;

/// advanced link, BS only
#[derive(Debug)]
pub struct TlReceiveInd;

// advanced link
#[derive(Debug)]
pub struct TlReleaseReq {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
}
#[derive(Debug)]
pub struct TlReleaseInd {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: Option<Todo>,
    pub endpoint_id: EndpointId,
}

/// advanced link
#[derive(Debug)]
pub struct TlReconnectReq;
/// advanced link
#[derive(Debug)]
pub struct TlReconnectResp;

// pub enum TlaReport {
//     /// Confirm handle to the request
//     ConfirmHandle,

// }

#[derive(Debug)]
pub struct TlaTlReportInd {
    pub req_handle: Option<Todo>,
    pub report: Todo,
    pub chan_change_resp_req: Option<bool>,
    pub chan_change_handle: Option<Todo>,
    pub chan_info: Option<Todo>,
    pub endpoint_id: Option<Todo>,
}

/// Clause 20.3.5.1.9
/// TL-UNITDATA request: this primitive shall be used in the unacknowledged data transfer service by the layer 2
/// service user to request layer 2 to transmit a TL-SDU.
#[derive(Debug)]
pub struct TlaTlUnitdataReqBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub tl_sdu: BitBuffer,
    pub scrambling_code: Todo,
    pub pdu_prio: Todo,
    pub stealing_permission: bool,
    pub subscriber_class: Todo,
    pub fcs_flag: bool,
    pub air_interface_encryption: Todo,
    pub data_prio: Todo,
    pub packet_data_flag: bool,
    pub n_tlsdu_repeats: Option<Todo>,
    pub scheduled_data_status: Todo,
    pub max_schedule_interval: Option<Todo>,
    pub data_class_info: Option<Todo>,
    pub req_handle: Todo,
}

/// Clause 20.3.5.1.9
/// TL-UNITDATA indication: this primitive shall be used in the unacknowledged data transfer service to deliver
/// the received TL-SDU to the layer 2 service user.
#[derive(Debug)]
pub struct TlaTlUnitdataIndBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: Option<EndpointId>,
    pub css_endpoint_id: Option<EndpointId>,
    pub tl_sdu: Option<BitBuffer>,
    pub scrambling_code: u32,
    pub fcs_flag: bool,
    pub air_interface_encryption: Todo,
    pub chan_change_resp_req: bool,
    pub chan_change_handle: Option<Todo>,
    pub chan_info: Option<Todo>,
    pub report: Option<Todo>,
}

/// Clause 20.3.5.1.9, optional
/// TL-UNITDATA confirm: this primitive may be used in the unacknowledged data transfer service to indicate
/// completion of sending of the requested TL-SDU.
#[derive(Debug)]
pub struct TlUnitdataConfBl {
    // pub address_type: Todo,
    pub main_address: TetraAddress,
    pub link_id: LinkId,
    pub endpoint_id: EndpointId,
    pub req_handle: Todo,
    pub report: Option<Todo>,
}

/// Advanced link
#[derive(Debug)]
pub struct TlUnitdataReqAl;
/// Advanced link
#[derive(Debug)]
pub struct TlUnitdataIndAl;
/// Advanced link, optional?
#[derive(Debug)]
pub struct TlUnitdataConfAl;
