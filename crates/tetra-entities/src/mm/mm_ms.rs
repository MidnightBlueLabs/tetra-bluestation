use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{Sap, unimplemented_log};
use tetra_saps::{SapMsg, SapMsgInner};

use tetra_pdus::mm::enums::mm_pdu_type_dl::MmPduTypeDl;

pub struct MmMs {
    // config: Option<SharedConfig>,
    config: SharedConfig,
}

impl MmMs {
    pub fn new(config: SharedConfig) -> Self {
        Self { config }
    }

    fn rx_lmm_mle_unitdata_ind(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LmmMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let Some(bits) = prim.sdu.peek_bits(4) else {
            tracing::warn!("insufficient bits: {}", prim.sdu.dump_bin());
            return;
        };

        let Ok(pdu_type) = MmPduTypeDl::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, prim.sdu.dump_bin());
            return;
        };

        match pdu_type {
            MmPduTypeDl::DOtar => unimplemented_log!("DOtar"),
            MmPduTypeDl::DAuthentication => unimplemented_log!("DAuthentication"),
            MmPduTypeDl::DCkChangeDemand => unimplemented_log!("DCkChangeDemand"),
            MmPduTypeDl::DDisable => unimplemented_log!("DDisable"),
            MmPduTypeDl::DEnable => unimplemented_log!("DEnable"),
            MmPduTypeDl::DLocationUpdateAccept => unimplemented_log!("DLocationUpdateAccept"),
            MmPduTypeDl::DLocationUpdateCommand => unimplemented_log!("DLocationUpdateCommand"),
            MmPduTypeDl::DLocationUpdateReject => unimplemented_log!("DLocationUpdateReject"),
            MmPduTypeDl::DLocationUpdateProceeding => {
                unimplemented_log!("DLocationUpdateProceeding")
            }
            MmPduTypeDl::DAttachDetachGroupIdentity => {
                unimplemented_log!("DAttachDetachGroupIdentity")
            }
            MmPduTypeDl::DAttachDetachGroupIdentityAcknowledgement => {
                unimplemented_log!("DAttachDetachGroupIdentityAcknowledgement")
            }
            MmPduTypeDl::DMmStatus => unimplemented_log!("DMmStatus"),
            MmPduTypeDl::MmPduFunctionNotSupported => {
                unimplemented_log!("MmPduFunctionNotSupported")
            }
        };
    }
}

impl TetraEntityTrait for MmMs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Mm
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        // There is only one SAP for MM
        assert!(message.sap == Sap::LmmSap);

        match message.msg {
            SapMsgInner::LmmMleUnitdataInd(_) => {
                self.rx_lmm_mle_unitdata_ind(queue, message);
            }
            _ => {
                panic!();
            }
        }
    }
}
