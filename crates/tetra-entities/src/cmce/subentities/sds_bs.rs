use crate::MessageQueue;
use tetra_core::{BitBuffer, Sap, TetraAddress, address::SsiType, unimplemented_log};
use tetra_pdus::cmce::{enums::cmce_pdu_type_ul::CmcePduTypeUl, pdus::u_sds_data::USdsData, pdus::d_sds_data::DSdsData};
use tetra_saps::{SapMsg, SapMsgInner};
use tetra_saps::lcmc::LcmcMleUnitdataReq;
use tetra_saps::control::sds::BrewSdsRxInd;
use tetra_core::tetra_entities::TetraEntity;

/// Clause 13 Short Data Service CMCE sub-entity (BS side)
///
/// BS primarily receives **U-SDS-DATA** on the air interface and forwards the
/// SDS Type-4 payload (including PID octet) to Brew/Core.
///
/// NOTE: We DO NOT rewrite SDS-TL Message Reference (MR). MR is used end-to-end
/// (e.g. delivery reports) and must be preserved per ETSI.
pub struct SdsBsSubentity;

impl SdsBsSubentity {
    pub fn new() -> Self {
        SdsBsSubentity
    }
    fn rx_u_sds_data(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_u_sds_data");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };

        let pdu = match USdsData::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => pdu,
            Err(e) => {
                tracing::warn!("Failed parsing USdsData: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        // We currently forward only Type-4 (SDTI=3) since it carries a variable-length payload.
        if pdu.short_data_type_identifier != 3 {
            tracing::debug!("USdsData sdti={} (not type-4); ignoring", pdu.short_data_type_identifier);
            return;
        }

        let Some(bit_len) = pdu.length_indicator else {
            tracing::warn!("USdsData type-4 missing length_indicator");
            return;
        };
        let Some(payload) = pdu.user_defined_data_4 else {
            tracing::warn!("USdsData type-4 missing user_defined_data_4");
            return;
        };

        // Destination SSI
        let (dst_ssi, dst_type) = if let Some(ssi) = pdu.called_party_ssi {
            (ssi as u32, SsiType::Ssi)
        } else if let Some(short) = pdu.called_party_short_number_address {
            (short as u32, SsiType::Unknown)
        } else {
            tracing::warn!("USdsData missing called_party address");
            return;
        };



        tracing::info!(
            "SDS RX head: from={} dst={} head={:02x?}",
            prim.received_tetra_address,
            tetra_core::TetraAddress::new(dst_ssi, dst_type),
            payload.get(0..payload.len().min(8)).unwrap_or(&[])
        );
        // Interop: do not synthesize SDS-TL REPORTs here.
        // Many terminals accept only the destination-generated SDS-REPORT.
        // We relay destination reports end-to-end in BrewEntity.
        let ind = BrewSdsRxInd {
            source: prim.received_tetra_address,
            destination: tetra_core::TetraAddress::new(dst_ssi, dst_type),
            payload,
            bit_length: bit_len,
        };

        tracing::info!(
            "SDS RX: from={} dst={} bits={} bytes={}",
            ind.source,
            ind.destination,
            ind.bit_length,
            ind.payload.len()
        );

        // Forward to Brew entity via Control SAP.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Brew,
            dltime: message.dltime,
            msg: SapMsgInner::BrewSdsRxInd(ind),
        });
    }

    #[allow(dead_code)]
    fn send_sds_tl_report_to_source(
        &mut self,
        queue: &mut MessageQueue,
        dltime: tetra_core::TdmaTime,
        handle: u32,
        endpoint_id: u32,
        link_id: u32,
        source_addr: TetraAddress,
        report_sender_ssi: u32,
        raw4: [u8; 4],
    ) {
        // Build a minimal D-SDS-DATA Type-4 with 32-bit SDS-TL status payload (PID + type + status + MR)
        let pdu = DSdsData {
            calling_party_type_identifier: 1,
            calling_party_address_ssi: Some(report_sender_ssi as u64),
            calling_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(32),
            user_defined_data_4: Some(raw4.to_vec()),
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(96);
        if let Err(e) = pdu.to_bitbuf(&mut sdu) {
            tracing::warn!("SDS TX REPORT: encode failed: {:?}", e);
            return;
        }
        sdu.seek(0);

        tracing::info!(
            "SDS TX REPORT: to={} from_ssi={} raw4={:02x?}",
            source_addr,
            report_sender_ssi,
            raw4
        );

        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle,
                endpoint_id,
                link_id,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None,
                main_address: source_addr,
                tx_reporter: None,
            }),
        });
    }


    /// Poor man's rx_prim, as this is a subcomponent and not governed by the MessageRouter.
    pub fn route_xx_deliver(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("route_xx_deliver");

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
            CmcePduTypeUl::USdsData => self.rx_u_sds_data(queue, message),
            _ => unimplemented_log!("SdsBsSubentity route_xx_deliver {:?}", pdu_type),
        }
    }
}
