use crate::{MessageQueue, TetraEntityTrait, brew};
use tetra_config::SharedConfig;
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Sap, SsiType, TdmaTime, TetraAddress, assert_warn, unimplemented_log};
use tetra_saps::control::brew::{BrewSubscriberAction, MmSubscriberUpdate};
use tetra_saps::lmm::LmmMleUnitdataReq;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::mm::components::client_state::{MmClientMgr, MmClientState};
use crate::mm::components::not_supported::make_ul_mm_pdu_function_not_supported;
use tetra_pdus::mm::enums::location_update_type::LocationUpdateType;
use tetra_pdus::mm::enums::mm_pdu_type_ul::MmPduTypeUl;
use tetra_pdus::mm::enums::status_uplink::StatusUplink;
use tetra_pdus::mm::fields::group_identity_attachment::GroupIdentityAttachment;
use tetra_pdus::mm::fields::group_identity_downlink::GroupIdentityDownlink;
use tetra_pdus::mm::fields::group_identity_location_accept::GroupIdentityLocationAccept;
use tetra_pdus::mm::fields::group_identity_uplink::GroupIdentityUplink;
use tetra_pdus::mm::pdus::d_attach_detach_group_identity_acknowledgement::DAttachDetachGroupIdentityAcknowledgement;
use tetra_pdus::mm::pdus::d_location_update_accept::DLocationUpdateAccept;
use tetra_pdus::mm::pdus::u_attach_detach_group_identity::UAttachDetachGroupIdentity;
use tetra_pdus::mm::pdus::u_itsi_detach::UItsiDetach;
use tetra_pdus::mm::pdus::u_location_update_demand::ULocationUpdateDemand;
use tetra_pdus::mm::pdus::u_mm_status::UMmStatus;

pub struct MmBs {
    config: SharedConfig,
    pub client_mgr: MmClientMgr,
}

impl MmBs {
    pub fn new(config: SharedConfig) -> Self {
        Self {
            config,
            client_mgr: MmClientMgr::new(),
        }
    }

    fn emit_subscriber_update(
        &self,
        queue: &mut MessageQueue,
        dltime: TdmaTime,
        issi: u32,
        groups: Vec<u32>,
        action: BrewSubscriberAction,
    ) {
        // If brew is active, take all brew-routable groups and emit an update to brew entity
        if brew::is_active(&self.config) {
            let brew_groups = groups
                .iter()
                .filter(|gssi| brew::is_brew_routable(&self.config, **gssi))
                .copied()
                .collect::<Vec<u32>>();
            if !brew_groups.is_empty() {
                let brew_update = MmSubscriberUpdate {
                    issi,
                    groups: brew_groups,
                    action,
                };
                let msg = SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Mm,
                    dest: TetraEntity::Brew,
                    dltime,
                    msg: SapMsgInner::MmSubscriberUpdate(brew_update),
                };
                queue.push_back(msg);
            }
        }

        // Always emit an update to the Cmce entity
        let mm_update = MmSubscriberUpdate { issi, groups, action };
        let msg = SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Mm,
            dest: TetraEntity::Cmce,
            dltime,
            msg: SapMsgInner::MmSubscriberUpdate(mm_update),
        };
        queue.push_back(msg);
    }

    fn rx_u_itsi_detach(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_u_itsi_detach");
        let SapMsgInner::LmmMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match UItsiDetach::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing UItsiDetach: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        // Check if we can satisfy this request, print unsupported stuff
        if !Self::feature_check_u_itsi_detach(&pdu) {
            tracing::error!("Unsupported critical features in UItsiDetach");
            return;
        }

        let ssi = prim.received_address.ssi;
        let detached_client = self.client_mgr.remove_client(ssi);
        if let Some(client) = detached_client {
            if !client.groups.is_empty() {
                let groups: Vec<u32> = client.groups.iter().copied().collect();
                self.emit_subscriber_update(_queue, message.dltime, ssi, groups, BrewSubscriberAction::Deaffiliate);
            }
            self.emit_subscriber_update(_queue, message.dltime, ssi, Vec::new(), BrewSubscriberAction::Deregister);
        } else {
            tracing::warn!("Received UItsiDetach for unknown client with SSI: {}", ssi);
            // return;
        };
    }

    fn rx_u_location_update_demand(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_location_update_demand");
        let SapMsgInner::LmmMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match ULocationUpdateDemand::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing ULocationUpdateDemand: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        // Check if we can satisfy this request, print unsupported stuff
        if !Self::feature_check_u_location_update_demand(&pdu) {
            tracing::error!("Unsupported critical features in ULocationUpdateDemand");
            return;
        }

        // Handle Energy Saving Mode request
        // TODO FIXME this does not yet seem to be functional, and prevents the MS from remaining
        // properly registered.
        // let esi = if let Some(esm) = pdu.energy_saving_mode {
        //     if esm != EnergySavingMode::StayAlive {
        //         unimplemented_log!("Got req for EnergySavingMode {}, overriding with {}", esm, EnergySavingMode::StayAlive);
        //     }
        //     Some(EnergySavingInformation {
        //         energy_saving_mode: EnergySavingMode::StayAlive,
        //         frame_number: None,
        //         multiframe_number: None,
        //     })
        // } else {
        //     None
        // };
        let esi = None;

        // Try to register the client
        let issi = prim.received_address.ssi;
        let is_new = !self.client_mgr.client_is_known(issi);
        if is_new {
            match self.client_mgr.try_register_client(issi, true) {
                Ok(_) => {
                    self.emit_subscriber_update(queue, message.dltime, issi, Vec::new(), BrewSubscriberAction::Register);
                }
                Err(e) => {
                    tracing::warn!("Failed registering roaming MS {}: {:?}", issi, e);
                    // unimplemented_log!("Handle failed registration of roaming MS");
                    return;
                }
            }
        } else if let Err(e) = self.client_mgr.set_client_state(issi, MmClientState::Attached) {
            tracing::warn!("Failed updating roaming MS {}: {:?}", issi, e);
            return;
        }

        // Process optional GroupIdentityLocationDemand field
        let gila = if let Some(gild) = pdu.group_identity_location_demand {
            // Try to attach to requested groups, then build GroupIdentityLocationAccept element
            let accepted_groups = if let Some(giu) = &gild.group_identity_uplink {
                Some(self.try_attach_detach_groups(queue, message.dltime, issi, &giu))
            } else {
                None
            };
            let gila = GroupIdentityLocationAccept {
                group_identity_accept_reject: 0, // Accept
                group_identity_downlink: accepted_groups,
            };

            Some(gila)
        } else {
            // No GroupIdentityLocationAccept element present
            None
        };

        // Build D-LOCATION UPDATE ACCEPT pdu
        let pdu_response = DLocationUpdateAccept {
            location_update_accept_type: pdu.location_update_type, // Practically identical besides minor migration-related difference
            ssi: Some(issi as u64),
            address_extension: None,
            subscriber_class: None,
            energy_saving_information: esi,
            scch_information_and_distribution_on_18th_frame: None,
            new_registered_area: None,
            security_downlink: None,
            group_identity_location_accept: gila,
            default_group_attachment_lifetime: None,
            authentication_downlink: None,
            group_identity_security_related_information: None,
            cell_type_control: None,
            proprietary: None,
        };

        // Convert pdu to bits
        let pdu_len = 4 + 3 + 24 + 1 + 1 + 1; // Minimal lenght; may expand beyond this. 
        let mut sdu = BitBuffer::new_autoexpand(pdu_len);
        pdu_response.to_bitbuf(&mut sdu).unwrap(); // we want to know when this happens
        sdu.seek(0);
        tracing::debug!("-> {} sdu {}", pdu_response, sdu.dump_bin());

        // Build and submit response prim
        let addr = TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Ssi,
            ssi: issi,
        };
        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            dltime: message.dltime,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle: prim.handle,
                address: addr,
                layer2service: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
            }),
        };
        queue.push_back(msg);
    }

    fn rx_u_mm_status(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_u_mm_status");
        let SapMsgInner::LmmMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match UMmStatus::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing UItsiDetach: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        let handled = false; // Set to true for properly handled U-MM STATUS messages
        match pdu.status_uplink {
            StatusUplink::ChangeOfEnergySavingModeRequest
            | StatusUplink::ChangeOfEnergySavingModeResponse
            | StatusUplink::DualWatchModeRequest
            | StatusUplink::TerminatingDualWatchModeRequest
            | StatusUplink::ChangeOfDualWatchModeResponse
            | StatusUplink::StartOfDirectModeOperation
            | StatusUplink::MsFrequencyBandsInformation
            | StatusUplink::RequestToStartDmGatewayOperation
            | StatusUplink::RequestToContinuedmGatewayOperation
            | StatusUplink::RequestToStopDmGatewayOperation
            | StatusUplink::RequestToAddDmMsAddresses
            | StatusUplink::RequestToRemoveDmMsAddresses
            | StatusUplink::RequestToReplaceDmMsAddresses
            | StatusUplink::AcceptanceToRemovalOfDmMsAddresses
            | StatusUplink::AcceptanceToChangeRegistrationLabel
            | StatusUplink::AcceptanceToStopDmGatewayOperation => {
                unimplemented_log!("{:?}", pdu.status_uplink)
            }
            _ => {
                assert_warn!(false, "Unrecognized UMmStatus type {:?}", pdu.status_uplink);
            }
        }

        if !handled {
            // A fairly untested, best-effort way of sending a PDU not supported error back
            // Note that an MS is not required to really do anything with this message.
            let (sapmsg, debug_str) = make_ul_mm_pdu_function_not_supported(
                prim.handle,
                MmPduTypeUl::UMmStatus,
                Some((6, pdu.status_uplink.into())),
                prim.received_address.ssi,
                message.dltime,
            );
            tracing::debug!("-> {}", debug_str);
            queue.push_back(sapmsg);
        }
    }

    fn rx_u_attach_detach_group_identity(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_u_attach_detach_group_identity");
        let SapMsgInner::LmmMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let issi = prim.received_address.ssi;
        let pdu = match UAttachDetachGroupIdentity::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing UAttachDetachGroupIdentity: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        // Check if we can satisfy this request, print unsupported stuff
        if !Self::feature_check_u_attach_detach_group_identity(&pdu) {
            tracing::error!("Unsupported features in UAttachDetachGroupIdentity");
            return;
        }

        // If group_identity_attach_detach_mode == 1, we first detach all groups
        if pdu.group_identity_attach_detach_mode == true {
            let prior_groups: Vec<u32> = self
                .client_mgr
                .get_client_by_issi(issi)
                .map(|client| client.groups.iter().copied().collect())
                .unwrap_or_default();
            match self.client_mgr.client_detach_all_groups(issi) {
                Ok(_) => {
                    if !prior_groups.is_empty() {
                        self.emit_subscriber_update(queue, message.dltime, issi, prior_groups, BrewSubscriberAction::Deaffiliate);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed detaching all groups for MS {}: {:?}", issi, e);
                    return;
                }
            }
        }

        // Try to attach to requested groups, and retrieve list of accepted GroupIdentityDownlink elements
        // We can unwrap since we did compat check earlier
        let accepted_gid = self.try_attach_detach_groups(queue, message.dltime, issi, &pdu.group_identity_uplink.unwrap());

        // Build reply PDU
        let pdu_response = DAttachDetachGroupIdentityAcknowledgement {
            group_identity_accept_reject: 0, // Accept
            reserved: false,                 // TODO FIXME Guessed proper value of reserved field
            proprietary: None,
            group_identity_downlink: Some(accepted_gid),
            group_identity_security_related_information: None,
        };

        // Write to PDU
        let mut sdu = BitBuffer::new_autoexpand(32);
        pdu_response.to_bitbuf(&mut sdu).unwrap(); // We want to know when this happens
        sdu.seek(0);
        tracing::debug!("-> {:?} sdu {}", pdu_response, sdu.dump_bin());

        let addr = TetraAddress {
            encrypted: false,
            ssi_type: SsiType::Ssi,
            ssi: issi,
        };
        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            dltime: message.dltime,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle: prim.handle,
                address: addr,
                layer2service: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
            }),
        };
        queue.push_back(msg);
    }

    fn rx_lmm_mle_unitdata_ind(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        // unimplemented_log!("rx_lmm_mle_unitdata_ind for MM component");
        let SapMsgInner::LmmMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let Some(bits) = prim.sdu.peek_bits(4) else {
            tracing::warn!("insufficient bits: {}", prim.sdu.dump_bin());
            return;
        };

        let Ok(pdu_type) = MmPduTypeUl::try_from(bits) else {
            tracing::warn!("invalid pdu type: {} in {}", bits, prim.sdu.dump_bin());
            return;
        };

        match pdu_type {
            MmPduTypeUl::UAuthentication => unimplemented_log!("UAuthentication"),
            MmPduTypeUl::UItsiDetach => self.rx_u_itsi_detach(queue, message),
            MmPduTypeUl::ULocationUpdateDemand => self.rx_u_location_update_demand(queue, message),
            MmPduTypeUl::UMmStatus => self.rx_u_mm_status(queue, message),
            MmPduTypeUl::UCkChangeResult => unimplemented_log!("UCkChangeResult"),
            MmPduTypeUl::UOtar => unimplemented_log!("UOtar"),
            MmPduTypeUl::UInformationProvide => unimplemented_log!("UInformationProvide"),
            MmPduTypeUl::UAttachDetachGroupIdentity => self.rx_u_attach_detach_group_identity(queue, message),
            MmPduTypeUl::UAttachDetachGroupIdentityAcknowledgement => unimplemented_log!("UAttachDetachGroupIdentityAcknowledgement"),
            MmPduTypeUl::UTeiProvide => unimplemented_log!("UTeiProvide"),
            MmPduTypeUl::UDisableStatus => unimplemented_log!("UDisableStatus"),
            MmPduTypeUl::MmPduFunctionNotSupported => unimplemented_log!("MmPduFunctionNotSupported"),
        };
    }

    fn try_attach_detach_groups(
        &mut self,
        queue: &mut MessageQueue,
        dltime: TdmaTime,
        issi: u32,
        giu_vec: &Vec<GroupIdentityUplink>,
    ) -> Vec<GroupIdentityDownlink> {
        let mut accepted_groups = Vec::new();
        let mut aff_groups = Vec::new();
        let mut deaff_groups = Vec::new();

        for giu in giu_vec.iter() {
            if giu.gssi.is_none() || giu.vgssi.is_some() || giu.address_extension.is_some() {
                unimplemented_log!("Only support GroupIdentityUplink with address_type 0");
                continue;
            }

            let gssi = giu.gssi.unwrap(); // can't fail
            let is_detach = giu.group_identity_detachment_uplink.is_some();

            if is_detach {
                match self.client_mgr.client_group_attach(issi, gssi, false) {
                    Ok(changed) => {
                        if changed {
                            deaff_groups.push(gssi);
                        }
                        let gid = GroupIdentityDownlink {
                            group_identity_attachment: None,
                            group_identity_detachment_uplink: giu.group_identity_detachment_uplink,
                            gssi: Some(gssi),
                            address_extension: None,
                            vgssi: None,
                        };
                        accepted_groups.push(gid);
                    }
                    Err(e) => {
                        tracing::warn!("Failed detaching MS {} from group {}: {:?}", issi, gssi, e);
                    }
                }
            } else {
                match self.client_mgr.client_group_attach(issi, gssi, true) {
                    Ok(changed) => {
                        if changed {
                            aff_groups.push(gssi);
                        }
                        // We have added the client to this group. Add an entry to the downlink response
                        let gid = GroupIdentityDownlink {
                            group_identity_attachment: Some(GroupIdentityAttachment {
                                group_identity_attachment_lifetime: 3, // re-attach after location update
                                class_of_usage: giu.class_of_usage.unwrap_or(0),
                            }),
                            group_identity_detachment_uplink: None,
                            gssi: Some(gssi),
                            address_extension: None,
                            vgssi: None,
                        };
                        accepted_groups.push(gid);
                    }
                    Err(e) => {
                        tracing::warn!("Failed attaching MS {} to group {}: {:?}", issi, gssi, e);
                    }
                }
            }
        }

        if !aff_groups.is_empty() {
            self.emit_subscriber_update(queue, dltime, issi, aff_groups, BrewSubscriberAction::Affiliate);
        }
        if !deaff_groups.is_empty() {
            self.emit_subscriber_update(queue, dltime, issi, deaff_groups, BrewSubscriberAction::Deaffiliate);
        }

        accepted_groups
    }

    fn feature_check_u_itsi_detach(pdu: &UItsiDetach) -> bool {
        let supported = true;
        if pdu.address_extension.is_some() {
            unimplemented_log!("Unsupported address_extension present");
        };
        if pdu.proprietary.is_some() {
            unimplemented_log!("Unsupported proprietary present");
        };
        supported
    }

    fn feature_check_u_location_update_demand(pdu: &ULocationUpdateDemand) -> bool {
        let mut supported = true;
        if pdu.location_update_type != LocationUpdateType::RoamingLocationUpdating
            && pdu.location_update_type != LocationUpdateType::ItsiAttach
        {
            unimplemented_log!("Unsupported {}", pdu.location_update_type);
            supported = false;
        }
        if pdu.request_to_append_la == true {
            unimplemented_log!("Unsupported request_to_append_la == true");
            supported = false;
        }
        if pdu.cipher_control == true {
            unimplemented_log!("Unsupported cipher_control == true");
            supported = false;
        }
        if pdu.ciphering_parameters.is_some() {
            unimplemented_log!("Unsupported ciphering_parameters present");
            supported = false;
        }
        if pdu.class_of_ms.is_some() {
            unimplemented_log!("Unsupported class_of_ms present");
        }
        if pdu.energy_saving_mode.is_some() {
            unimplemented_log!("Unsupported energy_saving_mode present");
        }
        if pdu.la_information.is_some() {
            unimplemented_log!("Unsupported la_information present");
        }
        if pdu.ssi.is_some() {
            unimplemented_log!("Unsupported ssi present");
        }
        if pdu.address_extension.is_some() {
            unimplemented_log!("Unsupported address_extension present");
        }
        if pdu.group_report_response.is_some() {
            unimplemented_log!("Unsupported group_report_response present");
        }
        if pdu.authentication_uplink.is_some() {
            unimplemented_log!("Unsupported authentication_uplink present");
        }
        if pdu.extended_capabilities.is_some() {
            unimplemented_log!("Unsupported extended_capabilities present");
        }
        if pdu.proprietary.is_some() {
            unimplemented_log!("Unsupported proprietary present");
        }

        supported
    }

    /// Check for unsupported features in U-ATTACH/DETACH GROUP IDENTITY
    /// Returns false if a critical feature is missing
    fn feature_check_u_attach_detach_group_identity(pdu: &UAttachDetachGroupIdentity) -> bool {
        let mut supported = true;
        if pdu.group_identity_report == true {
            unimplemented_log!("Unsupported group_identity_report == true");
        }
        if pdu.group_identity_uplink.is_none() {
            unimplemented_log!("Missing group_identity_uplink");
            supported = false;
        }
        if pdu.group_report_response.is_some() {
            unimplemented_log!("Unsupported group_report_response present");
        }
        if pdu.proprietary.is_some() {
            unimplemented_log!("Unsupported proprietary present");
        }

        supported
    }
}

impl TetraEntityTrait for MmBs {
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
