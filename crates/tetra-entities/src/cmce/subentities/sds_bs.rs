use tetra_config::bluestation::SharedConfig;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity, unimplemented_log};
use tetra_pdus::cmce::enums::pre_coded_status::PreCodedStatus;
use tetra_pdus::cmce::enums::short_report_type::ShortReportType;
use tetra_saps::control::enums::sds_user_data::SdsUserData;
use tetra_saps::control::sds::CmceSdsData;
use tetra_saps::lcmc::LcmcMleUnitdataReq;
use tetra_saps::{SapMsg, SapMsgInner};

use tetra_pdus::cmce::enums::party_type_identifier::PartyTypeIdentifier;
use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;
use tetra_pdus::cmce::pdus::d_status::DStatus;
use tetra_pdus::cmce::pdus::u_sds_data::USdsData;
use tetra_pdus::cmce::pdus::u_status::UStatus;

use super::home_mode_display::HomeModeDisplaySender;
use crate::MessageQueue;
use crate::brew;

/// Clause 13 Short Data Service CMCE sub-entity
pub struct SdsBsSubentity {
    config: SharedConfig,
    home_mode_display_sender: HomeModeDisplaySender,
}

impl SdsBsSubentity {
    pub fn new(config: SharedConfig) -> Self {
        SdsBsSubentity {
            config,
            home_mode_display_sender: HomeModeDisplaySender::new(),
        }
    }

    pub fn tick_start(&mut self, queue: &mut MessageQueue, dltime: TdmaTime) {
        if let Some(home_mode_tx) = self.home_mode_display_sender.tick_start(&self.config, dltime) {
            self.send_d_sds_data(
                queue,
                dltime,
                home_mode_tx.source_issi,
                home_mode_tx.dest_gssi,
                SsiType::Gssi,
                home_mode_tx.payload,
            );
        }
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
                tracing::debug!("<- {:?}", pdu);
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
            pdu.user_defined_data.type_identifier()
        );

        // Route: local delivery (ISSI or GSSI), Brew forward, or drop
        let is_local_issi = self.config.state_read().subscribers.is_registered(dest_ssi);
        let is_local_group = !is_local_issi && self.config.state_read().subscribers.has_group_members(dest_ssi);

        if is_local_issi {
            tracing::info!("SDS: local delivery: {} -> {}", source_ssi, dest_ssi);
            self.send_d_sds_data(queue, message.dltime, source_ssi, dest_ssi, SsiType::Issi, pdu.user_defined_data);
        } else if is_local_group {
            tracing::info!("SDS: group delivery: {} -> GSSI {}", source_ssi, dest_ssi);
            self.send_d_sds_data(queue, message.dltime, source_ssi, dest_ssi, SsiType::Gssi, pdu.user_defined_data);
        } else if brew::feature_sds_enabled(&self.config)
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
                    user_defined_data: pdu.user_defined_data,
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
            sds.user_defined_data.type_identifier(),
            sds.user_defined_data.length_bits()
        );

        if !self.config.state_read().subscribers.is_registered(sds.dest_issi) {
            tracing::warn!("SDS: dest ISSI {} from Brew is not locally registered, dropping", sds.dest_issi);
            return;
        }

        // Send D-SDS-DATA downlink to the local MS. Schedule on next ts1 to ensure it gets sent on the MCCH
        self.send_d_sds_data(
            queue,
            message.dltime.forward_to_timeslot(1),
            sds.source_issi,
            sds.dest_issi,
            SsiType::Issi,
            sds.user_defined_data,
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
                tracing::debug!("<- {:?}", pdu);
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

        // Route: local delivery, Brew forward, or drop
        if self.config.state_read().subscribers.is_registered(dest_ssi) {
            tracing::info!("SDS-STATUS: local delivery: {} -> {}", source_ssi, dest_ssi);
            self.send_d_status(queue, message.dltime, source_ssi, dest_ssi, pdu.pre_coded_status);
        } else if brew::is_active(&self.config)
            && (brew::is_brew_issi_routable(&self.config, dest_ssi) || brew::is_tetrapack_sds_service_issi(&self.config, dest_ssi))
        {
            // Brew forwarding only: when the pre-coded status carries an SDS-TL short report
            // (ETSI 29.4.2.3), convert it to a full SDS-TL REPORT PDU (Type4) so the
            // remote end recognizes it as a delivery confirmation. ETSI 29.3.3.4.4
            // explicitly allows SwMI to "modify a short report to a standard report."
            // Non-SDS-TL pre-coded statuses are forwarded as-is (Type1).
            // Local delivery (D-STATUS) is not affected, it stays as pre-coded status above.
            let user_defined_data = if let PreCodedStatus::SdsTl(report) = &pdu.pre_coded_status {
                let delivery_status = match report.short_report_type() {
                    ShortReportType::MessageReceived => 0x00,
                    ShortReportType::MessageConsumed => 0x00,
                    ShortReportType::DestMemFull => 0x02,
                    ShortReportType::ProtOrEncodingNotSupported => 0x01,
                };
                // PID 0x82 = SDS-TL text messaging. Hardcoded because the SDS-SHORT REPORT
                // PDU does not carry a Protocol Identifier (ETSI 29.4.3.11). In practice
                // all observed SDS-TL traffic uses PID 0x82.
                let sds_tl_report = vec![0x82, 0x10, delivery_status, report.message_reference()];
                tracing::info!(
                    "SDS-STATUS: converting SDS-TL short report to Type4 for Brew: MR={} status=0x{:02x}",
                    report.message_reference(),
                    delivery_status
                );
                SdsUserData::Type4(32, sds_tl_report)
            } else {
                SdsUserData::Type1(pdu.pre_coded_status.into_raw())
            };

            tracing::info!("SDS-STATUS: forwarding to Brew: {} -> {}", source_ssi, dest_ssi);
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                dltime: message.dltime,
                msg: SapMsgInner::CmceSdsData(CmceSdsData {
                    source_issi: source_ssi,
                    dest_issi: dest_ssi,
                    user_defined_data,
                }),
            });
        } else {
            tracing::warn!(
                "SDS-STATUS: dest ISSI {} not locally registered and not Brew-routable, dropping",
                dest_ssi
            );
        }
    }

    /// Build and send a D-STATUS PDU to a local MS
    fn send_d_status(
        &self,
        queue: &mut MessageQueue,
        dltime: tetra_core::TdmaTime,
        source_issi: u32,
        dest_issi: u32,
        pre_coded_status: PreCodedStatus,
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
        user_defined_data: SdsUserData,
    ) {
        let pdu = DSdsData {
            calling_party_type_identifier: PartyTypeIdentifier::Ssi,
            calling_party_address_ssi: Some(source_issi as u64),
            calling_party_extension: None,
            user_defined_data,
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
