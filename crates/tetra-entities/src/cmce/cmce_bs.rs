use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{Sap, TdmaTime, unimplemented_log};
use tetra_saps::{SapMsg, SapMsgInner};

use tetra_pdus::cmce::enums::cmce_pdu_type_ul::CmcePduTypeUl;

use super::subentities::cc_bs::CcBsSubentity;
use super::subentities::sds_bs::SdsBsSubentity;
use super::subentities::ss_bs::SsBsSubentity;

pub struct CmceBs {
    config: SharedConfig,

    cc: CcBsSubentity,
    sds: SdsBsSubentity,
    ss: SsBsSubentity,
}

impl CmceBs {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            config: config.clone(),
            sds: SdsBsSubentity::new(),
            cc: CcBsSubentity::new(config.clone()),
            ss: SsBsSubentity::new(),
        }
    }

    pub fn rx_lcmc_mle_unitdata_ind(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_lcmc_mle_unitdata_ind");

        // Handle the incoming unit data indication
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let Some(bits) = prim.sdu.peek_bits(5) else {
            tracing::warn!("insufficient bits: {}", prim.sdu.dump_bin());
            return;
        };
        let Ok(pdu_type) = CmcePduTypeUl::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, prim.sdu.dump_bin());
            return;
        };

        match pdu_type {
            CmcePduTypeUl::UAlert
            | CmcePduTypeUl::UConnect
            | CmcePduTypeUl::UDisconnect
            | CmcePduTypeUl::UInfo
            | CmcePduTypeUl::URelease
            | CmcePduTypeUl::USetup
            | CmcePduTypeUl::UStatus
            | CmcePduTypeUl::UTxCeased
            | CmcePduTypeUl::UTxDemand
            | CmcePduTypeUl::UCallRestore => {
                self.cc.route_xx_deliver(_queue, message);
            }
            CmcePduTypeUl::USdsData => {
                unimplemented_log!("{:?}", pdu_type);
                // self.sds.route_xx_deliver(_queue, message);
            }
            CmcePduTypeUl::UFacility => {
                unimplemented_log!("{:?}", pdu_type);
                // self.ss.route_xx_deliver(_queue, message);
            }
            CmcePduTypeUl::CmceFunctionNotSupported => {
                unimplemented_log!("{:?}", pdu_type);
            }
        };
    }
}

impl TetraEntityTrait for CmceBs {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Cmce
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, ts: TdmaTime) {
        // Testing code
        // if ts == TdmaTime::default().add_timeslots(10*18*4+2) {
        //     // Inject a call start
        //     self.cc.run_call_test(queue, ts);
        // }

        // Propagate tick to subentities
        self.cc.tick_start(queue, ts);
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        tracing::debug!("rx_prim: {:?}", message);
        // tracing::debug!(ts=%message.dltime, "rx_prim: {:?}", message);

        match message.sap {
            Sap::LcmcSap => match message.msg {
                SapMsgInner::LcmcMleUnitdataInd(_) => {
                    self.rx_lcmc_mle_unitdata_ind(queue, message);
                }
                _ => {
                    panic!("Unexpected message on LcmcSap: {:?}", message.msg);
                }
            },
            Sap::Control => match message.msg {
                SapMsgInner::CmceCallControl(_) => {
                    self.cc.rx_call_control(queue, message);
                }
                SapMsgInner::BrewSubscriberUpdate(update) => {
                    self.cc.handle_subscriber_update(queue, update);
                }
                _ => {
                    panic!("Unexpected control message: {:?}", message.msg);
                }
            },
            _ => {
                panic!("Unexpected SAP: {:?}", message.sap);
            }
        }
    }
}
