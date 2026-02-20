use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::SharedConfig;
use tetra_core::Sap;
use tetra_core::tetra_entities::TetraEntity;
use tetra_saps::{SapMsg, SapMsgInner};

use tetra_pdus::cmce::enums::cmce_pdu_type_dl::CmcePduTypeDl;

use super::subentities::cc_ms::CcMsSubentity;
use super::subentities::sds_ms::SdsMsSubentity;
use super::subentities::ss_ms::SsMsSubentity;

pub struct CmceMs {
    config: SharedConfig,

    sds: SdsMsSubentity,
    cc: CcMsSubentity,
    ss: SsMsSubentity,
}

impl CmceMs {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            config,
            sds: SdsMsSubentity::new(),
            cc: CcMsSubentity::new(),
            ss: SsMsSubentity::new(),
        }
    }

    pub fn rx_unitdata_ind(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_unitdata_ind");

        // Handle the incoming unit data indication
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let Some(bits) = prim.sdu.peek_bits(5) else {
            tracing::warn!("insufficient bits: {}", prim.sdu.dump_bin());
            return;
        };
        let Ok(pdu_type) = CmcePduTypeDl::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, prim.sdu.dump_bin());
            return;
        };

        match pdu_type {
            CmcePduTypeDl::DSdsData | CmcePduTypeDl::DStatus => {
                self.sds.route_rf_deliver(queue, message);
            }
            CmcePduTypeDl::DFacility => {
                self.ss.route_re_deliver(queue, message);
            }
            CmcePduTypeDl::DAlert
            | CmcePduTypeDl::DCallProceeding
            | CmcePduTypeDl::DCallRestore
            | CmcePduTypeDl::DConnect
            | CmcePduTypeDl::DConnectAcknowledge
            | CmcePduTypeDl::DDisconnect
            | CmcePduTypeDl::DInfo
            | CmcePduTypeDl::DRelease
            | CmcePduTypeDl::DSetup
            | CmcePduTypeDl::DTxCeased
            | CmcePduTypeDl::DTxContinue
            | CmcePduTypeDl::DTxGranted
            | CmcePduTypeDl::DTxInterrupt
            | CmcePduTypeDl::DTxWait => {
                self.cc.route_rd_deliver(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }
}

impl TetraEntityTrait for CmceMs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Cmce
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        // There is only one SAP for CMCE
        assert!(message.sap == Sap::LcmcSap);

        match message.msg {
            SapMsgInner::LcmcMleUnitdataInd(_) => {
                self.rx_unitdata_ind(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }
}
