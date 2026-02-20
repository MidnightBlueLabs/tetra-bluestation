use tetra_core::unimplemented_log;
use tetra_pdus::cmce::{enums::cmce_pdu_type_dl::CmcePduTypeDl, pdus::d_sds_data::DSdsData};
use tetra_saps::{SapMsg, SapMsgInner};

use crate::MessageQueue;

/// Clause 13 Short Data Service CMCE sub-entity
pub struct SdsMsSubentity {}

impl SdsMsSubentity {
    /// Create a new instance of the SdsSubentity
    pub fn new() -> Self {
        SdsMsSubentity {}
    }

    pub fn rx_sds_data(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_sds_data");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let _pdu = match DSdsData::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("Received DSdsData: {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing DSdsData: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        unimplemented_log!("rx_sds_data not implemented");
    }

    /// Poor man's rx_prim, as this is a subcomponent and not governed by the MessageRouter
    /// If need be, we can deviate from the standard's subentity ranking and make this a full-fledged component
    /// See Figure 14.2: Block view of CMCE-MS
    pub fn route_rf_deliver(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("route_rf_deliver");

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

        // TODO FIXME: Besides these PDUs, we can also receive several signals (BUSY ind, CLOSE ind, etc)
        match pdu_type {
            CmcePduTypeDl::DSdsData => {
                self.rx_sds_data(queue, message);
            }
            CmcePduTypeDl::DStatus => {
                unimplemented_log!("rx_prim not implemented for SDS DStatus PDU");
            }
            _ => {
                panic!();
            }
        }
    }
}
