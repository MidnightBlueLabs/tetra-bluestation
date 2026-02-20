use tetra_core::unimplemented_log;
use tetra_pdus::cmce::enums::cmce_pdu_type_dl::CmcePduTypeDl;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::MessageQueue;

/// Clause 11 Call Control CMCE sub-entity
pub struct CcMsSubentity {}

impl CcMsSubentity {
    pub fn new() -> Self {
        CcMsSubentity {}
    }

    pub fn route_rd_deliver(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("route_rd_deliver");

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
            CmcePduTypeDl::DAlert => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DCallProceeding => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DCallRestore => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DConnect => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DConnectAcknowledge => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DDisconnect => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DInfo => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DRelease => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DSetup => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DTxCeased => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DTxContinue => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DTxGranted => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DTxInterrupt => {
                unimplemented_log!("{}", pdu_type);
            }
            CmcePduTypeDl::DTxWait => {
                unimplemented_log!("{}", pdu_type);
            }
            _ => {
                panic!();
            }
        }
    }
}
