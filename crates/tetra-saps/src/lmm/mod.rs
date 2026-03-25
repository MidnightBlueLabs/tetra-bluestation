// Clause 17.3.2 Service primitives for the LMM-SAP
#![allow(unused)]
use tetra_core::{BitBuffer, Layer2Service, MleHandle, TetraAddress, Todo, TxReporter};

/// This shall be used as a request to initiate the selection of a cell for communications. The
/// request shall always be made after power on and may be made at any time thereafter.
#[derive(Debug, Clone)]
pub struct LmmMleActivateReq {
    pub mcc_list: Vec<u16>,
    pub mnc_list: Vec<u16>,
    pub la_list: Vec<u16>,
    pub cell_type_prefs: Option<Todo>,
}

#[derive(Debug, Clone)]
pub struct LmmMleActivateInd {
    pub cell_availability: Todo,
}

/// This shall be used as a confirmation to the MM entity that a cell has been selected with the
/// required characteristics.
#[derive(Debug, Clone)]
pub struct LmmMleActivateConf {
    pub registration_required: bool,
    pub la: u16,
    pub cell_type: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleActivityReq {
    pub sleep_mode: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleBusyReq {}

#[derive(Debug, Clone)]
pub struct LmmMleCancelReq {
    pub handle: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleCloseReq {}
#[derive(Debug, Clone)]
pub struct LmmMleConfigureReq {
    pub periodic_reporting_timer: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleConfigureInd {
    pub periodic_reporting_timer: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleDeactivateReq {}

#[derive(Debug, Clone)]
pub struct LmmMleDisableReq {
    pub permitted_services_in_temp_disabled_mode: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleEnableReq {}

#[derive(Debug, Clone)]
pub struct LmmMleIdentitiesReq {
    pub issi: Todo,
    pub assi: Todo,
    pub attached_gssis: Vec<Todo>,
    pub detached_gssis: Vec<Todo>,
}

#[derive(Debug, Clone)]
pub struct LmmMleIdleReq {}

#[derive(Debug, Clone)]
pub struct LmmMleInfoReq {
    pub subscriber_class: Todo,
    pub scch_config: Todo,
    pub energy_economy_config: Todo,
    pub minimal_mode_config: Todo,
    pub dual_watch_config: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleInfoInd {
    pub broadcast_params: Todo,
    pub subscriber_class_match: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleLinkReq {
    pub mcc: Todo,
    pub mnc: Todo,
    pub la_list: Vec<u16>,
    pub cell_type_prefs: Option<Todo>,
}

#[derive(Debug, Clone)]
pub struct LmmMleLinkInd {
    pub mcc: Todo,
    pub mnc: Todo,
    pub la: u16,
    pub registration_type: Todo,
    pub security_params: Todo,
    pub cell_type: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleOpen {}

#[derive(Debug, Clone)]
pub struct LmmMlePrepareReq {
    pub sdu: Todo,
    pub handle: Todo,
    pub layer2service: Layer2Service,
    pub pdu_prio: Todo,
    pub stealing_permission: bool,
    pub stealing_repeats_flag: bool,
}

#[derive(Debug, Clone)]
pub struct LmmMlePrepareConfirm {
    pub sdu: Todo,
    pub handle: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleReportInd {
    pub handle: MleHandle,
    pub transfer_result: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleUnitdataReq {
    pub sdu: BitBuffer,
    pub handle: MleHandle,
    // pub address_type: Todo,
    pub address: TetraAddress,
    pub layer2service: Layer2Service,
    // pub pdu_prio: Todo, // Optional feature
    pub stealing_permission: bool,
    pub stealing_repeats_flag: bool,
    pub encryption_flag: bool,
    pub is_null_pdu: bool, // Prio should be lowest and may not steal
    pub tx_reporter: Option<TxReporter>,
}

#[derive(Debug, Clone)]
pub struct LmmMleUnitdataInd {
    pub sdu: BitBuffer,
    pub handle: MleHandle,
    pub received_address: TetraAddress,
    // pub received_address_type: Todo,
}

#[derive(Debug, Clone)]
pub struct LmmMleUpdateReq {
    pub mcc: Todo,
    pub mnc: Todo,
    pub ra: Todo,
    pub cell_type_prefs: Option<Todo>,
    pub registration_result: Todo,
}
