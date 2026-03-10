use tetra_config::bluestation::SharedConfig;
use tetra_core::{BitBuffer, Sap, SsiType, TetraAddress, tetra_entities::TetraEntity, unimplemented_log};
use tetra_saps::control::sds::CmceSdsData;
use tetra_saps::lcmc::LcmcMleUnitdataReq;
use tetra_saps::{SapMsg, SapMsgInner};

use tetra_pdus::cmce::enums::party_type_identifier::PartyTypeIdentifier;
use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;
use tetra_pdus::cmce::pdus::d_status::DStatus;
use tetra_pdus::cmce::pdus::u_sds_data::USdsData;
use tetra_pdus::cmce::pdus::u_status::UStatus;

use crate::MessageQueue;
use crate::brew;

/// Clause 13 Short Data Service CMCE sub-entity
pub struct SdsBsSubentity {
    config: SharedConfig,
}

impl SdsBsSubentity {
    pub fn new(config: SharedConfig) -> Self {
        SdsBsSubentity { config }
    }

    /// Handle incoming U-SDS-DATA from a local MS (via RF uplink)
    pub fn route_rf_deliver(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("SDS route_rf_deliver");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let calling_party = prim.received_tetra_address;

        let pdu = match USdsData::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- U-SDS-DATA {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-SDS-DATA: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        if !Self::feature_check_u_sds_data(&pdu) {
            tracing::warn!("Unsupported features in U-SDS-DATA, dropping");
            return;
        }

        // Extract destination SSI (guaranteed present after feature check)
        let dest_ssi = pdu.called_party_ssi.unwrap() as u32;

        let source_ssi = calling_party.ssi;

        tracing::info!(
            "SDS: U-SDS-DATA from ISSI {} to ISSI {}, type={}",
            source_ssi,
            dest_ssi,
            pdu.short_data_type_identifier
        );

        // Extract SDS payload into a uniform representation
        let (data, length_bits) = match pdu.short_data_type_identifier {
            0 => {
                // Type 1: 16 bits
                let val = pdu.user_defined_data_1.unwrap_or(0);
                (val.to_be_bytes()[6..8].to_vec(), 16u16)
            }
            1 => {
                // Type 2: 32 bits
                let val = pdu.user_defined_data_2.unwrap_or(0);
                (val.to_be_bytes()[4..8].to_vec(), 32u16)
            }
            2 => {
                // Type 3: 64 bits
                let val = pdu.user_defined_data_3.unwrap_or(0);
                (val.to_be_bytes().to_vec(), 64u16)
            }
            3 => {
                // Type 4: variable length
                let len_bits = pdu.length_indicator.unwrap_or(0) as u16;
                let data = pdu.user_defined_data_4.unwrap_or_default();
                (data, len_bits)
            }
            _ => {
                tracing::warn!("SDS: invalid short_data_type_identifier={}", pdu.short_data_type_identifier);
                return;
            }
        };

        // Route: local delivery (ISSI or GSSI), Brew forward, or drop
        let is_local_issi = self.config.state_read().subscribers.is_registered(dest_ssi);
        let is_local_group = !is_local_issi && self.config.state_read().subscribers.has_group_members(dest_ssi);

        if is_local_issi {
            tracing::info!("SDS: local delivery: {} -> {}", source_ssi, dest_ssi);
            self.send_d_sds_data(
                queue,
                message.dltime,
                source_ssi,
                dest_ssi,
                SsiType::Issi,
                pdu.short_data_type_identifier,
                &data,
                length_bits,
            );
        } else if is_local_group {
            tracing::info!("SDS: group delivery: {} -> GSSI {}", source_ssi, dest_ssi);
            self.send_d_sds_data(
                queue,
                message.dltime,
                source_ssi,
                dest_ssi,
                SsiType::Gssi,
                pdu.short_data_type_identifier,
                &data,
                length_bits,
            );
        } else if brew::is_active(&self.config)
            && (brew::is_brew_issi_routable(&self.config, dest_ssi) || brew::is_tetrapack_sds_service_issi(&self.config, dest_ssi))
        {
            tracing::info!("SDS: forwarding to Brew: {} -> {}", source_ssi, dest_ssi);
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                dltime: message.dltime,
                msg: SapMsgInner::CmceSdsData(CmceSdsData {
                    source_issi: source_ssi,
                    dest_issi: dest_ssi,
                    short_data_type_identifier: pdu.short_data_type_identifier,
                    data,
                    length_bits,
                }),
            });
        } else {
            tracing::warn!("SDS: dest SSI {} not local and not Brew-routable, dropping", dest_ssi);
        }
    }

    /// Handle incoming SDS data from Brew entity (network-originated SDS)
    pub fn rx_sds_from_brew(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        let SapMsgInner::CmceSdsData(sds) = message.msg else {
            panic!("Expected CmceSdsData message");
        };

        tracing::info!(
            "SDS: received from Brew: {} -> {}, type={}, {} bits",
            sds.source_issi,
            sds.dest_issi,
            sds.short_data_type_identifier,
            sds.length_bits
        );

        if !self.config.state_read().subscribers.is_registered(sds.dest_issi) {
            tracing::warn!("SDS: dest ISSI {} from Brew is not locally registered, dropping", sds.dest_issi);
            return;
        }

        // Send D-SDS-DATA downlink to the local MS
        self.send_d_sds_data(
            queue,
            message.dltime,
            sds.source_issi,
            sds.dest_issi,
            SsiType::Issi,
            sds.short_data_type_identifier,
            &sds.data,
            sds.length_bits,
        );
    }

    /// Handle incoming U-STATUS from a local MS (via RF uplink)
    pub fn route_status_deliver(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("SDS route_status_deliver");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!();
        };
        let calling_party = prim.received_tetra_address;

        let pdu = match UStatus::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- U-STATUS {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-STATUS: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        if !Self::feature_check_u_status(&pdu) {
            tracing::warn!("Unsupported features in U-STATUS, dropping");
            return;
        }

        // Extract destination SSI (guaranteed present after feature check)
        let dest_ssi = pdu.called_party_ssi.unwrap() as u32;

        let source_ssi = calling_party.ssi;

        tracing::info!(
            "SDS: U-STATUS from ISSI {} to ISSI {}, status={}",
            source_ssi,
            dest_ssi,
            pdu.pre_coded_status
        );

        // Route: local delivery only (status is individual point-to-point)
        if self.config.state_read().subscribers.is_registered(dest_ssi) {
            tracing::info!("SDS-STATUS: local delivery: {} -> {}", source_ssi, dest_ssi);
            self.send_d_status(queue, message.dltime, source_ssi, dest_ssi, pdu.pre_coded_status);
        } else {
            tracing::warn!("SDS-STATUS: dest ISSI {} not locally registered, dropping", dest_ssi);
        }
    }

    /// Build and send a D-STATUS PDU to a local MS
    fn send_d_status(
        &self,
        queue: &mut MessageQueue,
        dltime: tetra_core::TdmaTime,
        source_issi: u32,
        dest_issi: u32,
        pre_coded_status: u16,
    ) {
        let pdu = DStatus {
            calling_party_type_identifier: PartyTypeIdentifier::Ssi,
            calling_party_address_ssi: Some(source_issi as u64),
            calling_party_extension: None,
            pre_coded_status,
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        tracing::debug!("-> D-STATUS {:?}", pdu);

        let mut sdu = BitBuffer::new_autoexpand(64);
        if let Err(e) = pdu.to_bitbuf(&mut sdu) {
            tracing::error!("Failed to serialize D-STATUS: {:?}", e);
            return;
        }
        sdu.seek(0);

        let dest_addr = TetraAddress::new(dest_issi, SsiType::Issi);
        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 0,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None,
                main_address: dest_addr,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);
    }

    /// Build and send a D-SDS-DATA PDU to a local MS
    fn send_d_sds_data(
        &self,
        queue: &mut MessageQueue,
        dltime: tetra_core::TdmaTime,
        source_issi: u32,
        dest_issi: u32,
        dest_ssi_type: SsiType,
        short_data_type_identifier: u8,
        data: &[u8],
        length_bits: u16,
    ) {
        // Build D-SDS-DATA PDU
        let (user_defined_data_1, user_defined_data_2, user_defined_data_3, length_indicator, user_defined_data_4) =
            match short_data_type_identifier {
                0 => {
                    let val = if data.len() >= 2 {
                        ((data[0] as u64) << 8) | (data[1] as u64)
                    } else if data.len() == 1 {
                        (data[0] as u64) << 8
                    } else {
                        0
                    };
                    (Some(val), None, None, None, None)
                }
                1 => {
                    let mut val: u64 = 0;
                    for (i, &b) in data.iter().take(4).enumerate() {
                        val |= (b as u64) << (24 - i * 8);
                    }
                    (None, Some(val), None, None, None)
                }
                2 => {
                    let mut val: u64 = 0;
                    for (i, &b) in data.iter().take(8).enumerate() {
                        val |= (b as u64) << (56 - i * 8);
                    }
                    (None, None, Some(val), None, None)
                }
                3 => (None, None, None, Some(length_bits as u64), Some(data.to_vec())),
                _ => {
                    tracing::warn!("SDS: invalid short_data_type_identifier={}", short_data_type_identifier);
                    return;
                }
            };

        let pdu = DSdsData {
            calling_party_type_identifier: PartyTypeIdentifier::Ssi,
            calling_party_address_ssi: Some(source_issi as u64),
            calling_party_extension: None,
            short_data_type_identifier,
            user_defined_data_1,
            user_defined_data_2,
            user_defined_data_3,
            length_indicator,
            user_defined_data_4,
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        tracing::debug!("-> D-SDS-DATA {:?}", pdu);

        let mut sdu = BitBuffer::new_autoexpand(128);
        if let Err(e) = pdu.to_bitbuf(&mut sdu) {
            tracing::error!("Failed to serialize D-SDS-DATA: {:?}", e);
            return;
        }
        sdu.seek(0);

        let dest_addr = TetraAddress::new(dest_issi, dest_ssi_type);
        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 0,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None,
                main_address: dest_addr,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);
    }

    fn feature_check_u_sds_data(pdu: &USdsData) -> bool {
        let mut supported = true;
        if pdu.called_party_ssi.is_none() {
            if pdu.called_party_short_number_address.is_some() {
                unimplemented_log!("SDS: short number addressing not supported");
            } else {
                tracing::warn!("SDS: no destination address in U-SDS-DATA");
            }
            supported = false;
        }
        if pdu.called_party_extension.is_some() {
            unimplemented_log!("SDS: TSI extension addressing not supported");
        }
        if pdu.external_subscriber_number.is_some() {
            unimplemented_log!("SDS: external_subscriber_number not supported");
        }
        if pdu.dm_ms_address.is_some() {
            unimplemented_log!("SDS: dm_ms_address not supported");
        }
        supported
    }

    fn feature_check_u_status(pdu: &UStatus) -> bool {
        let mut supported = true;
        if pdu.called_party_ssi.is_none() {
            if pdu.called_party_short_number_address.is_some() {
                unimplemented_log!("SDS-STATUS: short number addressing not supported");
            } else {
                tracing::warn!("SDS-STATUS: no destination address in U-STATUS");
            }
            supported = false;
        }
        if pdu.called_party_extension.is_some() {
            unimplemented_log!("SDS-STATUS: TSI extension addressing not supported");
        }
        if pdu.external_subscriber_number.is_some() {
            unimplemented_log!("SDS-STATUS: external_subscriber_number not supported");
        }
        if pdu.dm_ms_address.is_some() {
            unimplemented_log!("SDS-STATUS: dm_ms_address not supported");
        }
        supported
    }
}
