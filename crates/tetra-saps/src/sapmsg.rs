use core::fmt::Display;

use tetra_core::Sap;
use tetra_core::TdmaTime;
use tetra_core::tetra_entities::TetraEntity;

use crate::control::brew::MmSubscriberUpdate;
use crate::control::call_control::CallControl;
use crate::tmd::TmdCircuitDataInd;
use crate::tmd::TmdCircuitDataReq;
use crate::tnmm::TnmmTestDemand;
use crate::tnmm::TnmmTestResponse;

use super::lcmc::*;
use super::lmm::*;
use super::ltpd::*;
use super::tla::*;
use super::tlmb::*;
use super::tlmc::*;
use super::tma::*;
use super::tmv::*;
use super::tp::*;

/// Exhaustive list of SapMsgType structs for use in the SapMsg struct
/// See Clause 19.2.1 for an overview of all lower-layer SAPs
#[derive(Debug)]
pub enum SapMsgInner {
    // TODO FIXME and all that stuff
    // PhyControlUpdateNetinfo(PhyControlUpdateNetinfo),

    // LmacControlUpdateNetinfo(LmacControlUpdateNetinfo),
    /// TP-SAP (Contents not defined in standard)
    TpUnitdataInd(TpUnitdataInd),
    TpUnitdataReq(TpUnitdataReqSlot),

    // TMV-SAP
    TmvUnitdataReq(TmvUnitdataReqSlot),
    TmvUnitdataInd(TmvUnitdataInd),
    TmvConfigureReq(TmvConfigureReq),
    TmvConfigureConf(TmvConfigureConf),

    // TMA-SAP
    TmaUnitdataInd(TmaUnitdataInd),
    TmaUnitdataReq(TmaUnitdataReq),
    TmaReportInd(TmaReportInd),

    // TMB-SAP / TLB-SAP (merged to TLMB-SAP)
    TlmbSyncInd(TlmbSyncInd),
    TlmbSysinfoInd(TlmbSysinfoInd),

    // TMC-SAP
    TlmcConfigureReq(TlmcConfigureReq),

    // TMD-SAP (Uplane traffic and signalling)
    TmdCircuitDataReq(TmdCircuitDataReq),
    TmdCircuitDataInd(TmdCircuitDataInd),

    // TLB-SAP
    // TlmbSyncInd(TlmbSyncInd),
    // TlmbSysinfoInd(TlmbSysinfoInd),

    // TLA-SAP
    TlaTlDataIndBl(TlaTlDataIndBl),
    TlaTlDataReqBl(TlaTlDataReqBl),
    TlaTlReportInd(TlaTlReportInd),
    TlaTlUnitdataIndBl(TlaTlUnitdataIndBl),
    TlaTlUnitdataReqBl(TlaTlUnitdataReqBl),

    // LMM-SAP (MLE-MM)
    LmmMleUnitdataInd(LmmMleUnitdataInd),
    LmmMleUnitdataReq(LmmMleUnitdataReq),

    // LCMC-SAP (MLE-CMCE)
    LcmcMleUnitdataInd(LcmcMleUnitdataInd),
    LcmcMleUnitdataReq(LcmcMleUnitdataReq),

    // CMCE -> UMAC control
    CmceCallControl(CallControl),

    // MM -> Brew/CMCE subscriber update
    MmSubscriberUpdate(MmSubscriberUpdate),

    // LTPD-SAP (MLE-LTPD)
    LtpdMleUnitdataInd(LtpdMleUnitdataInd),

    // TNMM-SAP (MM-User)
    TnmmTestDemand(TnmmTestDemand),
    TnmmTestResponse(TnmmTestResponse),
}

impl Display for SapMsgInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            // TP-SAP
            // SapMsgInner::TpUnitdataInd(_) => write!(f, "TpUnitdataInd"),

            // TMV-SAP
            SapMsgInner::TmvUnitdataReq(_) => write!(f, "TmvUnitdataReq"),
            SapMsgInner::TmvUnitdataInd(_) => write!(f, "TmvUnitdataInd"),
            SapMsgInner::TmvConfigureReq(_) => write!(f, "TmvConfigureReq"),
            SapMsgInner::TmvConfigureConf(_) => write!(f, "TmvConfigureConf"),

            // TMA-SAP
            SapMsgInner::TmaUnitdataInd(_) => write!(f, "TmaUnitdataInd"),
            SapMsgInner::TmaUnitdataReq(_) => write!(f, "TmaUnitdataReq"),

            // TMB-SAP
            SapMsgInner::TlmbSyncInd(_) => write!(f, "TmbSyncInd"),
            SapMsgInner::TlmbSysinfoInd(_) => write!(f, "TmbSysinfoInd"),

            // Control/Brew
            SapMsgInner::MmSubscriberUpdate(_) => write!(f, "MmSubscriberUpdate"),

            // TLB-SAP
            // SapMsgInner::TlbTlSyncInd(_) => write!(f, "TlbTlSyncInd"),
            // SapMsgInner::TlbTlSysinfoInd(_) => write!(f, "TlbTlSysinfoInd"),
            _ => panic!("Unknown SapMsgInner type"),
        }
    }
}

#[derive(Debug)]
pub struct SapMsg {
    pub sap: Sap,
    // pub prim: SapPrim,
    // pub subprim: SapSubPrim,
    pub src: TetraEntity,
    pub dest: TetraEntity,
    /// Downlink time at the time the message was created
    pub dltime: TdmaTime,
    // pub t_action: TdmaTime,
    pub msg: SapMsgInner,
}

impl SapMsg {
    pub fn new(
        sap: Sap,
        // prim: SapPrim,
        // subprim: SapSubPrim,
        src: TetraEntity,
        dest: TetraEntity,
        t_submit: TdmaTime,
        // t_action: TdmaTime,
        msg: SapMsgInner,
    ) -> Self {
        Self {
            sap,
            // prim,
            // subprim,
            src,
            dest,
            dltime: t_submit,
            // t_action,
            msg,
        }
    }

    pub fn get_source(&self) -> &TetraEntity {
        &self.src
    }
    pub fn get_dest(&self) -> &TetraEntity {
        &self.dest
    }
    pub fn get_sap(&self) -> &Sap {
        &self.sap
    }
    // pub fn get_prim(&self) -> &SapPrim {
    //     &self.prim
    // }
    // pub fn get_subprim(&self) -> &SapSubPrim {
    //     &self.subprim
    // }
}
