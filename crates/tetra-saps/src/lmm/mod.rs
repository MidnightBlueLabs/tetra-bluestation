// Clause 17.3.2 Service primitives for the LMM-SAP
#![allow(unused)]
use tetra_core::{BitBuffer, MleHandle, TetraAddress, Todo};

/// This shall be used as a request to initiate the selection of a cell for communications. The
/// request shall always be made after power on and may be made at any time thereafter.
#[derive(Debug)]
pub struct LmmMleActivateReq {
    pub mcc_list: Vec<u16>,
    pub mnc_list: Vec<u16>,
    pub la_list: Vec<u16>,
    pub cell_type_prefs: Option<Todo>,
}

#[derive(Debug)]
pub struct LmmMleActivateInd {
    pub cell_availability: Todo,
}

/// This shall be used as a confirmation to the MM entity that a cell has been selected with the
/// required characteristics.
#[derive(Debug)]
pub struct LmmMleActivateConf {
    pub registration_required: bool,
    pub la: u16,
    pub cell_type: Todo,
}

#[derive(Debug)]
pub struct LmmMleActivityReq {
    pub sleep_mode: Todo,
}

#[derive(Debug)]
pub struct LmmMleBusyReq {}

#[derive(Debug)]
pub struct LmmMleCancelReq {
    pub handle: Todo,
}

#[derive(Debug)]
pub struct LmmMleCloseReq {}
#[derive(Debug)]
pub struct LmmMleConfigureReq {
    pub periodic_reporting_timer: Todo,
}

#[derive(Debug)]
pub struct LmmMleConfigureInd {
    pub periodic_reporting_timer: Todo,
}

#[derive(Debug)]
pub struct LmmMleDeactivateReq {}

#[derive(Debug)]
pub struct LmmMleDisableReq {
    pub permitted_services_in_temp_disabled_mode: Todo,
}

#[derive(Debug)]
pub struct LmmMleEnableReq {}

#[derive(Debug)]
pub struct LmmMleIdentitiesReq {
    pub issi: Todo,
    pub assi: Todo,
    pub attached_gssis: Vec<Todo>,
    pub detached_gssis: Vec<Todo>,
}

#[derive(Debug)]
pub struct LmmMleIdleReq {}

#[derive(Debug)]
pub struct LmmMleInfoReq {
    pub subscriber_class: Todo,
    pub scch_config: Todo,
    pub energy_economy_config: Todo,
    pub minimal_mode_config: Todo,
    pub dual_watch_config: Todo,
}

#[derive(Debug)]
pub struct LmmMleInfoInd {
    pub broadcast_params: Todo,
    pub subscriber_class_match: Todo,
}

#[derive(Debug)]
pub struct LmmMleLinkReq {
    pub mcc: Todo,
    pub mnc: Todo,
    pub la_list: Vec<u16>,
    pub cell_type_prefs: Option<Todo>,
}

#[derive(Debug)]
pub struct LmmMleLinkInd {
    pub mcc: Todo,
    pub mnc: Todo,
    pub la: u16,
    pub registration_type: Todo,
    pub security_params: Todo,
    pub cell_type: Todo,
}

#[derive(Debug)]
pub struct LmmMleOpen {}

#[derive(Debug)]
pub struct LmmMlePrepareReq {
    pub sdu: Todo,
    pub handle: Todo,
    pub layer2service: Todo,
    pub pdu_prio: Todo,
    pub stealing_permission: bool,
    pub stealing_repeats_flag: bool,
}

#[derive(Debug)]
pub struct LmmMlePrepareConfirm {
    pub sdu: Todo,
    pub handle: Todo,
}

#[derive(Debug)]
pub struct LmmMleReportInd {
    pub handle: MleHandle,
    pub transfer_result: Todo,
}

#[derive(Debug)]
pub struct LmmMleUnitdataReq {
    pub sdu: BitBuffer,
    pub handle: MleHandle,
    // pub address_type: Todo,
    pub address: TetraAddress,
    pub layer2service: Todo,
    // pub pdu_prio: Todo, // Optional feature
    pub stealing_permission: bool,
    pub stealing_repeats_flag: bool,
    pub encryption_flag: bool,
    pub is_null_pdu: bool, // Prio should be lowest and may not steal
}

#[derive(Debug)]
pub struct LmmMleUnitdataInd {
    pub sdu: BitBuffer,
    pub handle: MleHandle,
    pub received_address: TetraAddress,
    // pub received_address_type: Todo,
}

#[derive(Debug)]
pub struct LmmMleUpdateReq {
    pub mcc: Todo,
    pub mnc: Todo,
    pub ra: Todo,
    pub cell_type_prefs: Option<Todo>,
    pub registration_result: Todo,
}
