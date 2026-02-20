// Clause 17.3.5 Service state diagram for the LTPD-SAP (MLE-SNDCP)

#![allow(unused)]
use tetra_core::{BitBuffer, EndpointId, LinkId, TetraAddress, Todo};

#[derive(Debug)]
pub struct LtpdMleActivityReq {
    pub sleep_mode: bool,
}

#[derive(Debug)]
pub struct LtpdMleBreakInd {}

#[derive(Debug)]
pub struct LtpdMleBusyInd {}

#[derive(Debug)]
pub struct LtpdMleCancelReq {
    pub handle: Todo,
}

#[derive(Debug)]
pub struct LtpdMleCloseInd {}

#[derive(Debug)]
pub struct LtpdMleConfigureReq {
    pub chan_change_accepted: bool,
    pub chan_change_handle: Todo,
    pub call_release: Todo,
    pub endpoint_id: EndpointId,
    pub encryption_flag: bool,
    pub ms_default_data_prio: Todo,
    pub layer2_data_prio_lifetime: Todo,
    pub layer2_data_prio_signalling_delay: Todo,
    pub data_prio_random_access_delay_factor: Todo,
    pub data_class_info: Todo,
    pub schedule_repetition_info: Todo,
    pub sndcp_status: Todo,
}

#[derive(Debug)]
pub struct LtpdMleConfigureInd {
    pub endpoint_id: EndpointId,
    pub chan_change_responce_required: bool,
    pub chan_change_handle: Todo,
    pub reason_for_config_indication: Todo,
    pub conflicting_endpoint_id: EndpointId,
}

#[derive(Debug)]
pub struct LtpdMleConnectReq {
    pub address: Todo,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub reservation_info: Todo,
    pub pdu_prio: Todo,
    pub layer2_qos: Todo,
    pub encryption_flag: bool,
    pub setup_report: Todo,
}

#[derive(Debug)]
pub struct LtpdMleConnectInd {
    pub address: Todo,
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub layer2_qos: Todo,
    pub encryption_flag: bool,
    pub chan_change_resp_req: bool,
    pub chan_change_handle: Option<Todo>,
    pub setup_report: Todo,
}

#[derive(Debug)]
pub struct LtpdMleConnectResp {
    pub address: Todo,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub pdu_prio: Todo,
    pub stealing_permission: bool,
    pub layer2_qos: Todo,
    pub encryption_flag: bool,
    pub setup_report: Todo,
}

#[derive(Debug)]
pub struct LtpdMleConnectConfirm {
    pub address: Todo,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub layer2_qos: Todo,
    pub encryption_flag: bool,
    pub channel_change_resp_req: bool,
    pub channel_change_handle: Todo,
    pub setup_report: Todo,
}

#[derive(Debug)]
pub struct LtpdMleDisableInd {
    pub permitted_services_in_temp_disabled_mode: Todo,
}

#[derive(Debug)]
pub struct LtpdMleDisconnectReq {
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub pdu_prio: Todo,
    pub encryption_flag: bool,
    pub report: Todo,
}

#[derive(Debug)]
pub struct LtpdMleDisconnectInd {
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub encryption_flag: bool,
    pub chan_change_resp_req: bool,
    pub chan_change_handle: Option<Todo>,
    pub report: Todo,
}

#[derive(Debug)]
pub struct LtpdMleEnableInd {}

#[derive(Debug)]
pub struct LtpdMleInfoInd {
    pub broadcast_params: Todo,
    pub subscriber_class_match: Todo,
    pub schedule_timing_prompt: Todo,
    pub permitted_cell_info: Todo,
}

#[derive(Debug)]
pub struct LtpdMleIdleInd {}

#[derive(Debug)]
pub struct LtpdMleOpenInd {
    pub mcc: Todo, // Current network
    pub mnc: Todo, // Current network
}

#[derive(Debug)]
pub struct LtpdMleReceiveInd {
    pub endpoint_id: EndpointId,
    pub received_tetra_address: Todo, // ITSI/GSSI
    pub received_address_type: Todo,
}

#[derive(Debug)]
pub struct LtpdMleReconnectReq {
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub reservation_info: Todo,
    pub pdu_prio: Todo,
    pub encryption_flag: bool,
    pub stealing_permission: bool,
}

#[derive(Debug)]
pub struct LtpdMleReconnectConfirm {
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub encryption_flag: bool,
    pub report: Todo,
    pub reconnection_result: Todo,
}

#[derive(Debug)]
pub struct LtpdMleReconnectInd {
    pub endpoint_id: EndpointId,
    pub new_endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub encryption_flag: bool,
    pub report: Todo,
    pub reconnection_result: Todo,
}

#[derive(Debug)]
pub struct LtpdMleReleaseReq {
    pub link_id: LinkId,
}

#[derive(Debug)]
pub struct LtpdMleReportInd {
    pub handle: Todo,
    pub transfer_result: Todo,
}

#[derive(Debug)]
pub struct LtpdMleResumeInd {
    pub mcc: Todo, // Current network
    pub mnc: Todo, // Current network
}

#[derive(Debug)]
pub struct LtpdMleUnitdataReq {
    pub sdu: Todo,
    pub handle: Todo,
    pub layer2service: Todo,
    pub unacked_bl_repetitions: Todo,
    pub pdu_prio: Todo,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub stealing_permission: bool,
    pub stealing_repeats_flag: bool,
    pub channel_advice_flag: bool,
    pub data_class_info: Todo,
    pub data_prio: Todo,
    pub mle_data_prio_flag: bool,
    pub packet_data_flag: bool,
    pub scheduled_data_status: Todo,
    pub max_schedule_interval: Todo,
    pub fcs_flag: bool,
}

#[derive(Debug)]
pub struct LtpdMleUnitdataInd {
    pub sdu: BitBuffer,
    pub endpoint_id: EndpointId,
    pub link_id: LinkId,
    pub received_tetra_address: TetraAddress, // ITSI/GSSI
    pub chan_change_resp_req: bool,
    pub chan_change_handle: Option<Todo>,
}
