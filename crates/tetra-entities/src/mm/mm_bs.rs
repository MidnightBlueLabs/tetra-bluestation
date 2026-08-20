use std::collections::HashSet;

use crate::net_control::ControlEndpoint;
use crate::net_telemetry::channel::TelemetrySink;
use crate::{MessageQueue, TetraEntityTrait, net_brew};
use tetra_config::bluestation::{StackState, SharedConfig, Subscriber};
use tetra_core::tetra_entities::TetraEntity;
use tetra_core::{BitBuffer, Layer2Service, Sap, TdmaTime, TetraAddress, assert_warn, unimplemented_log};
use tetra_pdus::mm::fields::class_of_ms::ClassOfMs;
use tetra_saps::control::brew::{BrewSubscriberAction, MmSubscriberUpdate};
use tetra_saps::lmm::LmmMleUnitdataReq;
use tetra_saps::{SapMsg, SapMsgInner};

use crate::mm::components::not_supported::make_ul_mm_pdu_function_not_supported;
use tetra_pdus::mm::enums::energy_saving_mode::EnergySavingMode;
use tetra_pdus::mm::enums::location_update_type::LocationUpdateType;
use tetra_pdus::mm::enums::mm_pdu_type_ul::MmPduTypeUl;
use tetra_pdus::mm::enums::reject_cause::RejectCause;
use tetra_pdus::mm::enums::status_downlink::StatusDownlink;
use tetra_pdus::mm::enums::status_uplink::StatusUplink;
use tetra_pdus::mm::fields::energy_saving_information::EnergySavingInformation;
use tetra_pdus::mm::fields::group_identity_attachment::GroupIdentityAttachment;
use tetra_pdus::mm::fields::group_identity_downlink::GroupIdentityDownlink;
use tetra_pdus::mm::fields::group_identity_location_accept::GroupIdentityLocationAccept;
use tetra_pdus::mm::fields::group_identity_uplink::GroupIdentityUplink;
use tetra_pdus::mm::pdus::d_attach_detach_group_identity_acknowledgement::DAttachDetachGroupIdentityAcknowledgement;
use tetra_pdus::mm::pdus::d_location_update_accept::DLocationUpdateAccept;
use tetra_pdus::mm::pdus::d_location_update_command::DLocationUpdateCommand;
use tetra_pdus::mm::pdus::d_location_update_reject::DLocationUpdateReject;
use tetra_pdus::mm::pdus::d_mm_status::DMmStatus;
use tetra_pdus::mm::pdus::u_attach_detach_group_identity::UAttachDetachGroupIdentity;
use tetra_pdus::mm::pdus::u_itsi_detach::UItsiDetach;
use tetra_pdus::mm::pdus::u_location_update_demand::ULocationUpdateDemand;
use tetra_pdus::mm::pdus::u_mm_status::UMmStatus;

pub struct MmBs {
    config: SharedConfig,
    state: StackState,
    telemetry: Option<TelemetrySink>,
    control: Option<ControlEndpoint>,
}

impl MmBs {
    pub fn new(config: SharedConfig, state: StackState, telemetry: Option<TelemetrySink>, control: Option<ControlEndpoint>) -> Self {
        Self {
            config,
            state,
            telemetry,
            control,
        }
    }

    fn state_is_registered(&self, issi: u32) -> bool {
        self.state.with_subscribers(|subscribers| subscribers.is_registered(issi))
    }

    fn state_has_group_members(&self, gssi: u32) -> bool {
        self.state
            .with_subscribers(|subscribers| subscribers.group_has_local_attached_mses(gssi))
    }

    fn state_register_subscriber(&self, issi: u32) {
        self.state.with_subscribers(|subscribers| {
            subscribers.register(issi);
        });
    }

    fn state_deregister_subscriber(&self, issi: u32) -> Option<Subscriber> {
        self.state.with_subscribers(|subscribers| subscribers.deregister(issi))
    }

    fn state_group_attach(&self, issi: u32, gssi: u32) {
        match self.state.with_subscribers(|subscribers| subscribers.group_attach(issi, gssi)) {
            None => {
                tracing::warn!("state_group_attach: could not attach SSI {} to GSSI {}", issi, gssi);
            }
            Some(false) => {
                tracing::warn!("state_group_attach: SSI {} already attached", issi,);
            }
            _ => {}
        }
    }

    fn state_group_detach(&self, issi: u32, gssi: u32) -> bool {
        let result = self.state.with_subscribers(|subscribers| subscribers.group_detach(issi, gssi));
        match result {
            Some(_) => true,
            None => {
                tracing::warn!(
                    "state_group_detach: SSI {} either not registered or not attached to GSSI {}",
                    issi,
                    gssi
                );
                false
            }
        }
    }

    fn state_group_detach_all(&self, issi: u32) -> Option<HashSet<u32>> {
        let (groups, detach_result) = self
            .state
            .with_subscribers(|s| (s.get_attached_groups(issi), s.group_detach_all(issi)));

        match (groups, detach_result) {
            (None, None) => {
                tracing::warn!("state_group_detach_all: SSI {} not registered", issi,);
                None
            }
            (g, _) => {
                assert!(g.is_some());
                g
            }
        }
    }

    /// Returns true on success or false if the subscriber is not registered.
    /// The shared ledger currently exposes no public mutator for these fields, so this
    /// method is intentionally a compatibility guard that warns and refuses unknown ISSIs.
    pub fn state_set_client_energy_saving_mode(&mut self, issi: u32, mode: EnergySavingMode) -> bool {
        let is_known = self.state.with_subscribers(|subscribers| subscribers.is_registered(issi));
        if !is_known {
            tracing::warn!("state_set_client_energy_saving_mode: ISSI {} not found in subscriber store", issi);
            return false;
        }

        // The current SubscriberStore API exposes no public field-level updater.
        // This keeps the prototype compatible without mutating a stale local cache.
        let _ = mode;
        true
    }

    /// Returns true on success or false if the subscriber is not registered.
    /// The shared ledger currently exposes no public mutator for these fields, so this
    /// method is intentionally a compatibility guard that warns and refuses unknown ISSIs.
    pub fn state_set_client_class_of_ms(&mut self, issi: u32, class: Option<ClassOfMs>) -> bool {
        let is_known = self.state.with_subscribers(|subscribers| subscribers.is_registered(issi));
        if !is_known {
            tracing::warn!("state_set_client_class_of_ms: ISSI {} not found in subscriber store", issi);
            return false;
        }

        // The current SubscriberStore API exposes no public field-level updater.
        // This keeps the prototype compatible without mutating a stale local cache.
        let _ = class;
        true
    }

    fn emit_subscriber_update(&self, queue: &mut MessageQueue, issi: u32, groups: Vec<u32>, action: BrewSubscriberAction) {
        tracing::warn!("emitting deprecated subscriber update {} {:?} {:?}", issi, groups, action);
        // If brew is active, forward subscriber updates to the Brew entity.
        // Register/Deregister must always be sent for brew-routable ISSIs,
        // even when there are no group affiliations yet. The Brew worker
        // decides whether to send REGISTER or REREGISTER based on its own state.
        // Affiliate/Deaffiliate only sent when there are brew-routable groups.
        if net_brew::is_active(&self.config) {
            let brew_groups = groups
                .iter()
                .filter(|gssi| net_brew::is_brew_gssi_routable(&self.config, **gssi))
                .copied()
                .collect::<Vec<u32>>();
            let should_send = match action {
                BrewSubscriberAction::Register | BrewSubscriberAction::Deregister => net_brew::is_brew_issi_routable(&self.config, issi),
                BrewSubscriberAction::Affiliate | BrewSubscriberAction::Deaffiliate => !brew_groups.is_empty(),
            };
            if should_send {
                let brew_update = MmSubscriberUpdate {
                    issi,
                    groups: brew_groups,
                    action,
                };
                let msg = SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Mm,
                    dest: TetraEntity::Brew,
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
            msg: SapMsgInner::MmSubscriberUpdate(mm_update),
        };
        queue.push_back(msg);
    }

    fn rx_u_itsi_detach(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
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
        match self.state_deregister_subscriber(ssi) {
            Some(client) => {
                // self.config.state_write().subscribers.deregister(ssi);
                if !client.attached_groups.is_empty() {
                    let groups: Vec<u32> = client.attached_groups.iter().copied().collect();
                    self.emit_subscriber_update(queue, ssi, groups, BrewSubscriberAction::Deaffiliate);
                }
                self.emit_subscriber_update(queue, ssi, Vec::new(), BrewSubscriberAction::Deregister);
            }
            None => {
                tracing::warn!("Received UItsiDetach for unknown client with SSI: {}", ssi);
            }
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

        // Migration not supported: ETSI 16.4.1.1 case b) requires identity exchange via
        // D-LOCATION-UPDATE-PROCEEDING which we don't implement. Reject with cause
        // "Migration not supported" (12, Table 16.81) so the MS can act on it.
        if pdu.location_update_type == LocationUpdateType::MigratingLocationUpdating
            || pdu.location_update_type == LocationUpdateType::ServiceRestorationMigratingLocationUpdating
        {
            tracing::warn!(
                "Rejecting migration request from SSI {}: {}",
                prim.received_address.ssi,
                pdu.location_update_type
            );
            Self::send_d_location_update_reject(
                queue,
                prim.received_address.ssi,
                prim.handle,
                pdu.location_update_type,
                pdu.address_extension,
            );
            return;
        }

        // Check if we can satisfy this request, print unsupported stuff
        if !Self::feature_check_u_location_update_demand(&pdu) {
            tracing::error!("Unsupported critical features in ULocationUpdateDemand");
            return;
        }

        // Handle Energy Saving Mode request (clause 23.7.6)
        // Always override to StayAlive. DL scheduler does not track per-MS monitoring
        // patterns, so non-StayAlive modes would cause missed downlink messages.
        // Per clause 16.7.1 NOTE 1: "The BS may allocate a different energy saving mode
        // than requested and the BS assumes that the allocated value will be used."
        let esi = if let Some(esm) = pdu.energy_saving_mode {
            if esm != EnergySavingMode::StayAlive {
                tracing::debug!(
                    "MS {} requested energy saving mode {:?}, overriding to StayAlive",
                    prim.received_address.ssi,
                    esm,
                );
            }
            Some(EnergySavingInformation {
                energy_saving_mode: EnergySavingMode::StayAlive,
                frame_number: None,
                multiframe_number: None,
            })
        } else {
            None
        };

        // Try to register the client
        let issi = prim.received_address.ssi;
        let handle = prim.handle;
        let is_new = !self.state_is_registered(issi);
        if is_new {
            self.state_register_subscriber(issi);
            self.emit_subscriber_update(queue, issi, Vec::new(), BrewSubscriberAction::Register);
        } else {
            tracing::warn!("Registered already-known MS {}", issi);
            return;
        }

        // Store energy saving mode in client state
        let esm = esi.as_ref().map(|e| e.energy_saving_mode).unwrap_or(EnergySavingMode::StayAlive);
        self.state_set_client_energy_saving_mode(issi, esm);

        // Process optional GroupIdentityLocationDemand field
        let has_groups = pdu.group_identity_location_demand.is_some();
        let gila = if let Some(gild) = pdu.group_identity_location_demand {
            // ETSI Table 16.49 (clause 16.10.17): mode=1 means "detach all currently
            // attached group identities and attach group identities defined in the
            // group identity uplink element."
            if gild.group_identity_attach_detach_mode == 1 {
                match self.state_group_detach_all(issi) {
                    Some(detached_groups) => {
                        let prior_groups: Vec<u32> = detached_groups.iter().copied().collect();
                        if !prior_groups.is_empty() {
                            self.emit_subscriber_update(queue, issi, prior_groups, BrewSubscriberAction::Deaffiliate);
                        }
                    }
                    None => {
                        tracing::warn!("state_group_detach_all: no groups were detached for SSI {}", issi);
                    }
                }
                // let prior_groups: Vec<u32> = self
                //     .client_mgr
                //     .get_client_by_issi(issi)
                //     .map(|client| client.groups.iter().copied().collect())
                //     .unwrap_or_default();
                // if let Err(e) = self.client_mgr.client_detach_all_groups(issi) {
                //     tracing::warn!("Failed detaching all groups for MS {}: {:?}", issi, e);
                // } else if !prior_groups.is_empty() {
                //     {
                //         for &gssi in &prior_groups {
                //             self.state_group_detach(issi, gssi);
                //         }
                //     }
                //     self.emit_subscriber_update(queue, issi, prior_groups, BrewSubscriberAction::Deaffiliate);
                // }
            }

            // Try to attach to requested groups, then build GroupIdentityLocationAccept element
            let accepted_groups = if let Some(giu) = &gild.group_identity_uplink {
                Some(self.try_attach_detach_groups(queue, issi, &giu))
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

        // Store and log class_of_ms
        if let Some(ref class) = pdu.class_of_ms {
            tracing::info!("MS {} class_of_ms: {}", issi, class);
            let _ = self.state_set_client_class_of_ms(issi, pdu.class_of_ms);
        }

        // Build D-LOCATION UPDATE ACCEPT pdu
        let pdu_response = DLocationUpdateAccept {
            location_update_accept_type: pdu.location_update_type,
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
        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle: prim.handle,
                address: TetraAddress::issi(issi),
                layer2service: Layer2Service::Acknowledged,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);

        // If this is an unknown returning radio (not ITSI attach), force it to
        // re-register with full group report via D-LOCATION UPDATE COMMAND
        if is_new && pdu.location_update_type != LocationUpdateType::ItsiAttach && !has_groups {
            tracing::info!("Sending D-LOCATION UPDATE COMMAND to returning MS {} to request group report", issi);
            Self::send_d_location_update_command(queue, issi, handle);
        }
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
                tracing::warn!("Failed parsing UMmStatus: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        let issi = prim.received_address.ssi;
        let handle = prim.handle;

        let mut handled = false;
        match pdu.status_uplink {
            StatusUplink::ChangeOfEnergySavingModeRequest => {
                // Parse energy saving mode from the sub-PDU payload
                let esm = if let Some(dep_info) = pdu.status_uplink_dependent_information {
                    // First 3 bits of the dependent information contain the energy saving mode
                    let dep_len = pdu.status_uplink_dependent_information_len.unwrap_or(0);
                    if dep_len >= 3 {
                        let mode_val = dep_info >> (dep_len - 3);
                        EnergySavingMode::try_from(mode_val).unwrap_or(EnergySavingMode::StayAlive)
                    } else {
                        EnergySavingMode::StayAlive
                    }
                } else {
                    EnergySavingMode::StayAlive
                };

                if esm != EnergySavingMode::StayAlive {
                    tracing::info!(
                        "MS {} requested energy saving mode change to {:?}, overriding to StayAlive",
                        issi,
                        esm
                    );
                } else {
                    tracing::info!("MS {} energy saving mode change request: StayAlive", issi);
                }

                // Store StayAlive (see clause 16.7.1 NOTE 1)
                // Respond with StayAlive
                let chosen_esm = EnergySavingMode::StayAlive;
                self.state_set_client_energy_saving_mode(issi, chosen_esm);
                let esi = EnergySavingInformation {
                    energy_saving_mode: chosen_esm,
                    frame_number: None,
                    multiframe_number: None,
                };
                Self::send_d_mm_status_energy_saving(queue, issi, handle, esi);
                handled = true;
            }
            StatusUplink::ChangeOfEnergySavingModeResponse => {
                // MS confirming a BS-initiated change
                let esm = if let Some(dep_info) = pdu.status_uplink_dependent_information {
                    let dep_len = pdu.status_uplink_dependent_information_len.unwrap_or(0);
                    if dep_len >= 3 {
                        // TODO FIXME we basically accept a non-prompted modechange if a misbehaving MS sends this "response"
                        let mode_val = dep_info >> (dep_len - 3);
                        EnergySavingMode::try_from(mode_val).unwrap_or(EnergySavingMode::StayAlive)
                    } else {
                        EnergySavingMode::StayAlive
                    }
                } else {
                    EnergySavingMode::StayAlive
                };

                tracing::info!("MS {} energy saving mode change response: {:?}", issi, esm);
                let _ = self.state_set_client_energy_saving_mode(issi, esm);
                handled = true;
            }
            StatusUplink::DualWatchModeRequest
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
                handle,
                MmPduTypeUl::UMmStatus,
                Some((6, pdu.status_uplink.into())),
                prim.received_address,
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
            if !self.state_is_registered(issi) {
                // Client unknown (e.g. never registered via location update).
                // Re-register so group attachment can proceed.
                // TODO FIXME this should not happen if we demand attachment first!
                tracing::warn!("unregistered MS tries to attach/detach group");
                self.state_register_subscriber(issi);
                self.emit_subscriber_update(queue, issi, Vec::new(), BrewSubscriberAction::Register);
            } else {
                // Client is known — detach all existing groups first
                let prior_groups: Vec<u32> = self.state_group_detach_all(issi).unwrap().iter().copied().collect();
                if !prior_groups.is_empty() {
                    self.emit_subscriber_update(queue, issi, prior_groups, BrewSubscriberAction::Deaffiliate);
                }
            }
        } else {
            tracing::warn!("unimplemented attach/detach mode");
        }

        // Try to attach to requested groups, and retrieve list of accepted GroupIdentityDownlink elements
        // We can unwrap since we did compat check earlier
        let accepted_gid = self.try_attach_detach_groups(queue, issi, &pdu.group_identity_uplink.unwrap());

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

        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle: prim.handle,
                address: TetraAddress::issi(issi),
                layer2service: Layer2Service::Acknowledged,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
                tx_reporter: None,
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
                self.state_group_detach(issi, gssi);
                deaff_groups.push(gssi);

                let gid = GroupIdentityDownlink {
                    group_identity_attachment: None,
                    group_identity_detachment_uplink: giu.group_identity_detachment_uplink,
                    gssi: Some(gssi),
                    address_extension: None,
                    vgssi: None,
                };
                accepted_groups.push(gid);
            } else {
                self.state_group_attach(issi, gssi);
                aff_groups.push(gssi);
                let gid = GroupIdentityDownlink {
                    group_identity_attachment: Some(GroupIdentityAttachment {
                        group_identity_attachment_lifetime: 1, // re-attach after ITSI attach (ETSI default per clause 16.4.2)
                        class_of_usage: giu.class_of_usage.unwrap_or(0),
                    }),
                    group_identity_detachment_uplink: None,
                    gssi: Some(gssi),
                    address_extension: None,
                    vgssi: None,
                };
                accepted_groups.push(gid);
            }
        }

        if !aff_groups.is_empty() {
            self.emit_subscriber_update(queue, issi, aff_groups, BrewSubscriberAction::Affiliate);
        }
        if !deaff_groups.is_empty() {
            self.emit_subscriber_update(queue, issi, deaff_groups, BrewSubscriberAction::Deaffiliate);
        }

        accepted_groups
    }

    /// Sends a D-LOCATION UPDATE COMMAND to force the radio to re-register
    /// with full group identity report
    fn send_d_location_update_command(queue: &mut MessageQueue, issi: u32, handle: u32) {
        let pdu = DLocationUpdateCommand {
            group_identity_report: true,
            cipher_control: false,
            ciphering_parameters: None,
            address_extension: None,
            cell_type_control: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(16);
        pdu.to_bitbuf(&mut sdu).unwrap();
        sdu.seek(0);
        tracing::debug!("-> DLocationUpdateCommand sdu {}", sdu.dump_bin());

        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle,
                address: TetraAddress::issi(issi),
                layer2service: Layer2Service::Acknowledged,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);
    }

    /// Sends a D-LOCATION UPDATE REJECT PDU (ETSI clause 16.9.2.9)
    fn send_d_location_update_reject(
        queue: &mut MessageQueue,
        issi: u32,
        handle: u32,
        location_update_type: LocationUpdateType,
        address_extension: Option<u64>,
    ) {
        let pdu = DLocationUpdateReject {
            location_update_type,
            reject_cause: RejectCause::MigrationNotSupported as u8,
            cipher_control: false,
            ciphering_parameters: None,
            // Echo back MNI if present, required for case b) per ETSI 16.4.1.1
            address_extension,
            cell_type_control: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(16);
        pdu.to_bitbuf(&mut sdu).unwrap();
        sdu.seek(0);
        tracing::debug!("-> {} sdu {}", pdu, sdu.dump_bin());

        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle,
                address: TetraAddress::issi(issi),
                layer2service: Layer2Service::Acknowledged,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);
    }

    /// Sends a D-MM-STATUS with ChangeOfEnergySavingModeResponse
    fn send_d_mm_status_energy_saving(queue: &mut MessageQueue, issi: u32, handle: u32, esi: EnergySavingInformation) {
        let pdu = DMmStatus {
            status_downlink: StatusDownlink::ChangeOfEnergySavingModeResponse,
            energy_saving_information: Some(esi),
        };

        let mut sdu = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut sdu).unwrap();
        sdu.seek(0);
        tracing::debug!("-> {} sdu {}", pdu, sdu.dump_bin());

        let msg = SapMsg {
            sap: Sap::LmmSap,
            src: TetraEntity::Mm,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LmmMleUnitdataReq(LmmMleUnitdataReq {
                sdu,
                handle,
                address: TetraAddress::issi(issi),
                layer2service: Layer2Service::Acknowledged,
                stealing_permission: false,
                stealing_repeats_flag: false,
                encryption_flag: false,
                is_null_pdu: false,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);
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
        if pdu.location_update_type == LocationUpdateType::MigratingLocationUpdating
            || pdu.location_update_type == LocationUpdateType::DisabledMsUpdating
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

    fn tick_start(&mut self, _queue: &mut MessageQueue, _ts: TdmaTime) {
        if let Some(cep) = &self.control {
            while let Some(cmd) = cep.try_recv() {
                match cmd {
                    // ControlCommand::CommandA { handle, parameter } => {
                    //     cep.respond(ControlResponse::CommandAResponse { handle, result: parameter * 2 });
                    // }
                    _ => {
                        panic!("Unsupported command {:?}", cmd);
                    }
                }
            }
        }
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
