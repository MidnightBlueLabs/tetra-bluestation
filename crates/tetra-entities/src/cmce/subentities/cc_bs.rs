use tetra_config::bluestation::{
    CircuitState, CircuitStreamDest, CircuitStreamSrc, MleRoute, NetworkCallRequest, SharedConfig, StackState, TetraCircuit,
};
use tetra_core::{BitBuffer, Direction, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity, unimplemented_log};
use tetra_core::{Layer2Service, TxReporter, TxState};
use tetra_pdus::cmce::enums::disconnect_cause::DisconnectCause;
use tetra_pdus::cmce::{
    enums::{
        call_timeout::CallTimeout, call_timeout_setup_phase::CallTimeoutSetupPhase, cmce_pdu_type_ul::CmcePduTypeUl,
        transmission_grant::TransmissionGrant,
    },
    fields::basic_service_information::BasicServiceInformation,
    pdus::{
        d_alert::DAlert, d_call_proceeding::DCallProceeding, d_connect::DConnect, d_connect_acknowledge::DConnectAcknowledge,
        d_release::DRelease, d_setup::DSetup, d_tx_ceased::DTxCeased, d_tx_granted::DTxGranted, u_alert::UAlert, u_connect::UConnect,
        u_disconnect::UDisconnect, u_release::URelease, u_setup::USetup, u_tx_ceased::UTxCeased, u_tx_demand::UTxDemand,
    },
};
use tetra_saps::{
    SapMsg, SapMsgInner,
    control::{
        brew::{BrewSubscriberAction, MmSubscriberUpdate},
        call_control::{CallControl, Circuit, CircuitDlMediaSource},
        call_signal::{BrewEvent, CmceEvent},
        enums::{circuit_mode_type::CircuitModeType, communication_type::CommunicationType},
    },
    lcmc::{
        LcmcMleUnitdataReq,
        enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment},
        fields::chan_alloc_req::CmceChanAllocReq,
    },
};

use crate::net_brew;
use crate::{
    MessageQueue,
    cmce::components::circuit_mgr::{CircuitMgr, CircuitMgrCmd, CircuitRequest},
};

/// Clause 11 Call Control CMCE sub-entity.
///
/// All call state lives in the global `CircuitStore` and `SubscriberStore`, reached through
/// `state`. `circuits` is the only handle allowed to create, mutate and destroy circuits, and
/// it also owns the cached D-SETUP PDUs used for late entry and teardown.
pub struct CcBsSubentity {
    config: SharedConfig,
    state: StackState,

    dltime: TdmaTime,
    circuits: CircuitMgr,
}

/// ===== Global circuit view queries =====
///
/// A call stays in the store until its circuit is actually torn down, which is a few frames
/// after the release starts. The queries below skip circuits in `Releasing` so that a call
/// already being torn down can never be re-keyed, reused or answered.
impl CcBsSubentity {
    /// Snapshot of a live circuit by call id.
    fn live_circuit(&self, call_id: u16) -> Option<TetraCircuit> {
        self.circuits.get_circuit_by_callid(call_id).filter(|c| !c.is_releasing())
    }

    /// Snapshot of the first live circuit matching `pred`.
    fn find_live_circuit(&self, pred: impl Fn(&TetraCircuit) -> bool) -> Option<TetraCircuit> {
        self.circuits.find_circuit(|c| !c.is_releasing() && pred(c))
    }

    /// True if any live circuit matches `pred`.
    fn any_live_circuit(&self, pred: impl Fn(&TetraCircuit) -> bool) -> bool {
        self.circuits.any_circuit(|c| !c.is_releasing() && pred(c))
    }

    /// Snapshot of a live individual (point-to-point) call.
    fn live_individual_circuit(&self, call_id: u16) -> Option<TetraCircuit> {
        self.live_circuit(call_id).filter(|c| c.is_individual_call())
    }

    /// Snapshot of a live group call.
    fn live_group_circuit(&self, call_id: u16) -> Option<TetraCircuit> {
        self.live_circuit(call_id).filter(|c| c.is_group_call())
    }

    /// True if the call id belongs to a live individual call.
    fn is_individual_call_id(&self, call_id: u16) -> bool {
        self.live_individual_circuit(call_id).is_some()
    }

    /// Verbose dump of the global circuit view, for diagnosing call state from the log alone.
    #[allow(dead_code)]
    fn dump_circuit_states(&self) {
        let (circuits, map) = self.state.with_circuits(|c| (c.get_circuits().clone(), c.get_timeslot_map()));
        tracing::warn!("--- CircuitStore: {} circuit(s), dltime={:?} ---", circuits.len(), self.dltime);
        for (call_id, circuit) in &circuits {
            tracing::warn!("    call_id={} {:?}", call_id, circuit);
        }
        tracing::warn!("  timeslot map dl={:?}", map.dl);
        tracing::warn!("  timeslot map ul={:?}", map.ul);
    }
}

impl CcBsSubentity {
    pub fn new(config: SharedConfig, state: StackState) -> Self {
        CcBsSubentity {
            config,
            circuits: CircuitMgr::new(state.clone()),
            state,
            dltime: TdmaTime::default(),
        }
    }

    pub fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    /// Tell Brew what happened to a call. The event carries nothing but the call reference:
    /// Brew reads the call itself from the global circuit view.
    fn signal_brew(queue: &mut MessageQueue, event: CmceEvent) {
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Brew,
            msg: SapMsgInner::CmceCallEvent(event),
        });
    }

    /// Hand the Brew session of a call being destroyed over to Brew, so it can still close it
    /// upstream on an event that names the call by id.
    fn put_closing_session(&self, call_id: u16, uuid: uuid::Uuid) {
        self.state.with_circuits(|c| c.put_closing_session(call_id, uuid));
    }

    /// Deposit the parameters of a call for Brew to pick up with the accompanying signal.
    fn put_network_request(&self, uuid: uuid::Uuid, request: NetworkCallRequest) {
        self.state.with_circuits(|c| c.put_network_request(uuid, request));
    }

    /// Collect the parameters Brew deposited for a signalled call.
    fn take_network_request(&self, uuid: uuid::Uuid) -> Option<NetworkCallRequest> {
        self.state.with_circuits(|c| c.take_network_request(uuid))
    }

    fn build_d_setup_prim(pdu: &DSetup, usage: u8, ts: u8, ul_dl: UlDlAssignment) -> (BitBuffer, CmceChanAllocReq) {
        let mut sdu = BitBuffer::new_autoexpand(80);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DSetup");
        sdu.seek(0);
        tracing::info!("-> {:?} sdu {}", pdu, sdu.dump_bin());

        // Construct ChanAlloc descriptor for the allocated timeslot
        let mut timeslots = [false; 4];
        timeslots[ts as usize - 1] = true;
        let chan_alloc = CmceChanAllocReq {
            usage: Some(usage),
            alloc_type: ChanAllocType::Replace,
            carrier: None,
            timeslots,
            ul_dl_assigned: ul_dl,
        };
        (sdu, chan_alloc)
    }

    fn build_sapmsg(
        sdu: BitBuffer,
        chan_alloc: Option<CmceChanAllocReq>,
        address: TetraAddress,
        layer2service: Layer2Service,
        reporter: Option<TxReporter>,
    ) -> SapMsg {
        // Construct prim
        SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 0,
                layer2service,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc,
                main_address: address,
                tx_reporter: reporter,
            }),
        }
    }

    fn build_sapmsg_stealing(sdu: BitBuffer, address: TetraAddress, ts: u8) -> SapMsg {
        // For FACCH stealing on traffic channel, must specify target timeslot
        let mut timeslots = [false; 4];
        timeslots[(ts - 1) as usize] = true;
        let chan_alloc = CmceChanAllocReq {
            usage: None,
            carrier: None,
            timeslots,
            alloc_type: ChanAllocType::Replace,
            ul_dl_assigned: UlDlAssignment::Both,
        };

        SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 0,
                layer2service: Layer2Service::Unacknowledged, // TODO FIXME check if indeed only unacked over STCH
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: true,
                stealing_repeats_flag: false,
                chan_alloc: Some(chan_alloc),
                main_address: address,
                tx_reporter: None,
            }),
        }
    }

    fn build_d_release_from_d_setup(d_setup_pdu: &DSetup, disconnect_cause: DisconnectCause) -> BitBuffer {
        let pdu = DRelease {
            call_identifier: d_setup_pdu.call_identifier,
            disconnect_cause,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DRelease");
        sdu.seek(0);
        tracing::info!("-> {:?} sdu {}", pdu, sdu.dump_bin());

        sdu
    }

    /// True if at least one MS on this cell is attached to the group, so a call to it has
    /// somebody to reach.
    fn has_listener(&self, gssi: u32) -> bool {
        self.state.with_subscribers(|s| s.group_has_local_attached_mses(gssi))
    }

    /// Tear down any group call to a GSSI that no longer has local listeners.
    fn drop_group_calls_if_unlistened(&mut self, queue: &mut MessageQueue, gssi: u32) {
        if self.has_listener(gssi) {
            return;
        }

        let to_drop: Vec<u16> = self
            .circuits
            .find_circuits(|c| c.is_group_call() && !c.is_releasing() && c.callee == gssi)
            .into_iter()
            .map(|c| c.call_id)
            .collect();

        for call_id in to_drop {
            tracing::info!("CMCE: dropping call_id={} gssi={} (no listeners)", call_id, gssi);
            self.release_call(queue, call_id, DisconnectCause::SwmiRequestedDisconnection);
        }
    }

    pub fn handle_subscriber_update(&mut self, queue: &mut MessageQueue, update: MmSubscriberUpdate) {
        let issi = update.issi;
        let groups = update.groups;

        match update.action {
            BrewSubscriberAction::Register => {
                // Registering an already known subscriber would wipe its group attachments,
                // so a repeat registration is a no-op.
                let known = self.state.with_subscribers(|s| {
                    let known = s.is_registered(issi);
                    if !known {
                        s.register(issi);
                    }
                    known
                });
                tracing::info!("CMCE: subscriber register issi={} known={}", issi, known);
            }
            BrewSubscriberAction::Deregister => {
                let existing = self.state.with_subscribers(|s| s.deregister(issi).map(|sub| sub.attached_groups));
                if let Some(existing) = existing {
                    for gssi in existing {
                        self.drop_group_calls_if_unlistened(queue, gssi);
                    }
                }
                tracing::info!("CMCE: subscriber deregister issi={}", issi);
            }
            BrewSubscriberAction::Affiliate => {
                let new_groups: Vec<u32> = self.state.with_subscribers(|s| {
                    groups
                        .into_iter()
                        .filter(|gssi| s.group_attach(issi, *gssi) == Some(true))
                        .collect()
                });

                if new_groups.is_empty() {
                    tracing::debug!("CMCE: affiliate ignored (no new groups) issi={}", issi);
                } else {
                    tracing::info!("CMCE: subscriber affiliate issi={} groups={:?}", issi, new_groups);
                }
            }
            BrewSubscriberAction::Deaffiliate => {
                // An unknown ISSI detaches nothing, but the requested groups are still
                // re-checked for listeners in case this is a stale attachment.
                let removed_groups: Vec<u32> = self.state.with_subscribers(|s| {
                    if !s.is_registered(issi) {
                        return groups;
                    }
                    groups.into_iter().filter(|gssi| s.group_detach(issi, *gssi).is_some()).collect()
                });

                if removed_groups.is_empty() {
                    tracing::debug!("CMCE: deaffiliate ignored (no matching groups) issi={}", issi);
                } else {
                    tracing::info!("CMCE: subscriber deaffiliate issi={} groups={:?}", issi, removed_groups);
                    for gssi in &removed_groups {
                        self.drop_group_calls_if_unlistened(queue, *gssi);
                    }
                }
            }
        }
    }

    fn send_d_call_proceeding(&mut self, queue: &mut MessageQueue, message: &SapMsg, pdu_request: &USetup, call_id: u16) {
        tracing::trace!("send_d_call_proceeding");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {
            panic!()
        };

        let pdu_response = DCallProceeding {
            call_identifier: call_id,
            call_time_out_set_up_phase: CallTimeoutSetupPhase::T10s,
            hook_method_selection: pdu_request.hook_method_selection,
            simplex_duplex_selection: pdu_request.simplex_duplex_selection,
            basic_service_information: None, // Only needed if different from requested
            call_status: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(25);
        pdu_response.to_bitbuf(&mut sdu).expect("Failed to serialize DCallProceeding");
        sdu.seek(0);
        tracing::info!("-> {:?} sdu {}", pdu_response, sdu.dump_bin());

        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: prim.handle,
                endpoint_id: prim.endpoint_id,
                link_id: prim.link_id,
                layer2service: Layer2Service::Acknowledged,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,

                chan_alloc: None,
                main_address: prim.received_tetra_address,
                tx_reporter: None,
            }),
        };
        queue.push_back(msg);
    }

    /// Put a traffic timeslot into service. `peer_ts` cross-routes the uplink of this slot to
    /// the downlink of another one, which a duplex call needs so both parties hear each other.
    fn signal_umac_circuit_open(queue: &mut MessageQueue, ts: u8, usage: u8, peer_ts: Option<u8>, dl_media_source: CircuitDlMediaSource) {
        let circuit = Circuit {
            direction: Direction::Both,
            ts,
            peer_ts,
            usage,
            // Only speech is supported for now, TETRA ACELP encoded, without E2EE.
            circuit_mode: CircuitModeType::TchS,
            speech_service: Some(0),
            etee_encrypted: false,
            dl_media_source,
        };
        let cmd = SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::Open(circuit)),
        };
        queue.push_back(cmd);
    }

    /// Take a traffic timeslot out of service.
    fn signal_umac_circuit_close(queue: &mut MessageQueue, ts: u8) {
        let cmd = SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::Close(Direction::Both, ts)),
        };
        queue.push_back(cmd);
    }

    fn rx_u_setup(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        tracing::trace!("rx_u_setup: {:?}", message);
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        let calling_party = prim.received_tetra_address;

        let pdu = match USetup::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-SETUP: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

        // Individual (point-to-point) or group (point-to-multipoint), per the
        // communication type the MS declares in the basic service information.
        // Individual calls run their own path and state map (ETSI 14.5.1).
        if pdu.basic_service_information.communication_type == CommunicationType::P2p {
            self.setup_individual_call(queue, &message, pdu, calling_party);
            return;
        }

        // Check if we can satisfy this request
        if !Self::feature_check_u_setup(&pdu) {
            tracing::error!("Unsupported critical features in USetup");
            return;
        }

        // Get destination GSSI (called party)
        let Some(dest_gssi) = pdu.called_party_ssi else {
            tracing::warn!("U-SETUP without called_party_ssi, ignoring");
            return;
        };
        let dest_gssi = dest_gssi as u32;
        let dest_addr = TetraAddress::new(dest_gssi, SsiType::Gssi);

        if !self.has_listener(dest_gssi) {
            tracing::info!(
                "CMCE: rejecting U-SETUP from issi={} to gssi={} (no listeners)",
                calling_party.ssi,
                dest_gssi
            );
            return;
        }

        // Extract UL message routing info (handle, link_id, endpoint_id) for
        // individually-addressed responses. These are needed so MLE can route
        // the response back to the correct radio via the established LLC link.
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {
            panic!()
        };
        let ul_handle = prim.handle;
        let ul_link_id = prim.link_id;
        let ul_endpoint_id = prim.endpoint_id;

        // Open the call. The caller is granted the floor straight away, so the circuit goes
        // live in Tx rather than waiting for anyone to answer.
        let Some(call_id) = self.circuits.allocate_circuit(CircuitRequest {
            caller: calling_party.ssi,
            callee: dest_gssi,
            comm_type: pdu.basic_service_information.communication_type,
            state: CircuitState::Tx(0),
            is_duplex: false,
            second_channel: false,
            over_brew: false,
            hook_on_off: pdu.hook_method_selection,
            is_local_origin: true,
            is_mobile_terminated: false,
            origin_brew_uuid: None,
            caller_route: MleRoute::new(ul_handle, ul_link_id, ul_endpoint_id),
            floor: Some(calling_party.ssi),
        }) else {
            tracing::error!("Failed to allocate circuit for U-SETUP: no free timeslot");
            return;
        };
        let circuit = self.circuits.get_circuit_by_callid(call_id).expect("just allocated");
        let ts = circuit.dl_ts();
        let usage = circuit.usage_id;

        tracing::info!(
            "rx_u_setup: call from ISSI {} to GSSI {} → ts={} call_id={} usage={}",
            calling_party.ssi,
            dest_gssi,
            ts,
            call_id,
            usage
        );

        // Signal UMAC to open DL+UL circuits
        Self::signal_umac_circuit_open(queue, ts, usage, None, CircuitDlMediaSource::LocalLoopback);

        // Build channel allocation timeslot mask for this call
        let mut timeslots = [false; 4];
        timeslots[ts as usize - 1] = true;

        // === 1) Send D-CALL-PROCEEDING to the calling MS (individually addressed) ===
        // This acknowledges the U-SETUP and keeps the radio from timing out.
        self.send_d_call_proceeding(queue, &message, &pdu, call_id);

        // === 2) Send D-CONNECT to the calling MS with Granted + channel allocation ===
        // This transitions the calling MS from "Call Setup" to "Active".
        // MUST be sent BEFORE the group D-SETUP so the radio receives it on MCCH.
        // Uses the correct MLE handle (not 0) so MLE routes it properly.
        let d_connect = DConnect {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: pdu.hook_method_selection,
            simplex_duplex_selection: pdu.simplex_duplex_selection,
            transmission_grant: TransmissionGrant::Granted,
            transmission_request_permission: false,
            call_ownership: true, // Calling MS is the call owner (ETSI 14.8.4)
            call_priority: None,
            basic_service_information: None,
            temporary_address: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        let mut connect_sdu = BitBuffer::new_autoexpand(30);
        d_connect.to_bitbuf(&mut connect_sdu).expect("Failed to serialize DConnect");
        connect_sdu.seek(0);
        tracing::info!("-> {:?} sdu {}", d_connect, connect_sdu.dump_bin());

        let connect_msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu: connect_sdu,
                handle: ul_handle,
                endpoint_id: ul_endpoint_id,
                link_id: ul_link_id,
                layer2service: Layer2Service::Unacknowledged,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: Some(CmceChanAllocReq {
                    usage: Some(usage),
                    alloc_type: ChanAllocType::Replace,
                    carrier: None,
                    timeslots,
                    ul_dl_assigned: UlDlAssignment::Both,
                }),
                main_address: calling_party,
                tx_reporter: None,
            }),
        };
        queue.push_back(connect_msg);

        // === 3) Send D-SETUP to group (broadcast on MCCH with channel allocation) ===
        // GrantedToOtherUser tells other group members that someone else has the floor.
        let d_setup = DSetup {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: pdu.hook_method_selection,
            simplex_duplex_selection: pdu.simplex_duplex_selection,
            basic_service_information: pdu.basic_service_information.clone(),
            transmission_grant: TransmissionGrant::GrantedToOtherUser,
            transmission_request_permission: false,
            call_priority: pdu.call_priority,
            notification_indicator: None,
            temporary_address: None,
            calling_party_address_ssi: Some(calling_party.ssi),
            calling_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        // Cache for late-entry re-sends and for building the D-RELEASE at teardown.
        self.circuits.cache_setup(call_id, d_setup, dest_addr);
        let d_setup_ref = &self.circuits.get_setup(call_id).unwrap().pdu;

        let (setup_sdu, setup_chan_alloc) = Self::build_d_setup_prim(d_setup_ref, usage, ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, Some(setup_chan_alloc), dest_addr, Layer2Service::Unacknowledged, None);
        queue.push_back(setup_msg);

        // Notify Brew entity about this local call if Brew is loaded and the SSI is cleared for Brew
        // It can then forward to TetraPack if the group is subscribed
        if net_brew::is_brew_gssi_routable(&self.config, dest_gssi) {
            Self::signal_brew(queue, CmceEvent::TxStart { call_id });
        }
    }

    /// True if the ISSI is registered on this cell, so we can reach it for a local call.
    fn is_individual_registered(&self, issi: u32) -> bool {
        self.state.with_subscribers(|s| s.is_registered(issi))
    }

    /// True if the ISSI is already a party to an individual call.
    fn issi_in_individual_call(&self, issi: u32) -> bool {
        self.any_live_circuit(|c| c.is_individual_call() && (c.caller == issi || c.callee == issi))
    }

    /// Duplex flag of an individual call, as negotiated on air. Over Brew a duplex call still
    /// runs on a single local slot, so this cannot be derived from the second channel.
    fn individual_is_duplex(&self, call_id: u16) -> bool {
        self.live_individual_circuit(call_id).map(|c| c.is_duplex).unwrap_or(false)
    }

    /// Send a downlink PDU to the calling party over its established LLC link.
    fn send_to_caller(&self, queue: &mut MessageQueue, call: &TetraCircuit, sdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: call.caller_route.handle,
                endpoint_id: call.caller_route.endpoint_id,
                link_id: call.caller_route.link_id,
                layer2service: Layer2Service::Unacknowledged,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc,
                main_address: call.caller_addr(),
                tx_reporter: None,
            }),
        });
    }

    /// Set up an individual (point-to-point) call. ETSI EN 300 392-2 clause 14.5.1.
    /// Local on-cell simplex call with either hook method, ISSI-addressed.
    fn setup_individual_call(&mut self, queue: &mut MessageQueue, message: &SapMsg, pdu: USetup, calling_party: TetraAddress) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {
            panic!()
        };
        let (handle, link_id, endpoint_id) = (prim.handle, prim.link_id, prim.endpoint_id);

        let calling_ssi = calling_party.ssi;
        let duplex = pdu.simplex_duplex_selection;
        let called_ssi = pdu.called_party_ssi.map(|s| s as u32);

        // A locally registered ISSI is reached on-air. Anything else (an off-cell ISSI or a
        // PBX/phone number) is reached over Brew if it is configured, otherwise rejected.
        let is_local = called_ssi.map(|s| self.is_individual_registered(s)).unwrap_or(false);
        if !is_local {
            if net_brew::is_active(&self.config) {
                self.setup_individual_call_over_brew(queue, message, &pdu, calling_party, duplex, handle, link_id, endpoint_id);
            } else {
                tracing::warn!("individual call to non-local target and no Brew, rejecting");
                self.reject_individual_setup(queue, message, DisconnectCause::CalledPartyNotReachable);
            }
            return;
        }
        let called_ssi = called_ssi.expect("local target has an ISSI");

        if self.issi_in_individual_call(calling_ssi) {
            tracing::warn!("calling ISSI {} already in a call, rejecting", calling_ssi);
            self.reject_individual_setup(queue, message, DisconnectCause::ConcurrentSetUpNotSupported);
            return;
        }
        if self.issi_in_individual_call(called_ssi) {
            tracing::warn!("called ISSI {} busy, rejecting", called_ssi);
            self.reject_individual_setup(queue, message, DisconnectCause::CalledPartyBusy);
            return;
        }

        let comm_type = pdu.basic_service_information.communication_type;
        let called_addr = TetraAddress::new(called_ssi, SsiType::Issi);
        let hook_on_off = pdu.hook_method_selection;

        // Initial permission to transmit. Duplex grants both parties (talk and receive at
        // once), no floor. Simplex names one speaker via the U-SETUP request to transmit bit
        // (ETSI Table 14.74): value 0 is the caller, value 1 the other party. A hook radio
        // sets it to let the called speak first. The hook method only drives alerting.
        let (floor_holder, called_grant) = if duplex {
            (None, TransmissionGrant::Granted)
        } else {
            let caller_first = !pdu.request_to_transmit_send_data;
            let holder = if caller_first { calling_ssi } else { called_ssi };
            let grant = if caller_first {
                TransmissionGrant::GrantedToOtherUser
            } else {
                TransmissionGrant::Granted
            };
            (Some(holder), grant)
        };

        // A duplex call needs a second channel so the called party can transmit at the same
        // time as the caller. Simplex shares one channel (both parties on the same slot).
        let Some(call_id) = self.circuits.allocate_circuit(CircuitRequest {
            caller: calling_ssi,
            callee: called_ssi,
            comm_type,
            state: CircuitState::Setup(0),
            is_duplex: duplex,
            second_channel: duplex,
            over_brew: false,
            hook_on_off,
            is_local_origin: true,
            is_mobile_terminated: false,
            origin_brew_uuid: None,
            caller_route: MleRoute::new(handle, link_id, endpoint_id),
            floor: floor_holder,
        }) else {
            tracing::error!("Failed to allocate circuit for individual U-SETUP: no free timeslot");
            self.reject_individual_setup(queue, message, DisconnectCause::CongestionInInfrastructure);
            return;
        };
        let circuit = self.circuits.get_circuit_by_callid(call_id).expect("just allocated");
        let caller_ts = circuit.ul_ts();
        let called_ts = circuit.callee_ts();
        let called_usage = circuit.callee_usage_id();

        tracing::info!(
            "individual call ISSI {} to ISSI {} ts={} called_ts={} call_id={} hook_on_off={} duplex={}",
            calling_ssi,
            called_ssi,
            caller_ts,
            called_ts,
            call_id,
            hook_on_off,
            duplex
        );

        // Open the traffic channel(s). For duplex, cross-link the two slots so each party's
        // uplink voice loops to the other party's downlink.
        if duplex {
            Self::signal_umac_circuit_open(
                queue,
                caller_ts,
                circuit.usage_id,
                Some(called_ts),
                CircuitDlMediaSource::LocalLoopback,
            );
            Self::signal_umac_circuit_open(queue, called_ts, called_usage, Some(caller_ts), CircuitDlMediaSource::LocalLoopback);
        } else {
            Self::signal_umac_circuit_open(queue, caller_ts, circuit.usage_id, None, CircuitDlMediaSource::LocalLoopback);
        }

        // D-CALL-PROCEEDING acknowledges the U-SETUP to the caller.
        self.send_d_call_proceeding(queue, message, &pdu, call_id);

        // D-SETUP to the called party. No channel allocation here: in a hangtime
        // (quasi-transmission-trunked) call ETSI Table 14.1 does not allow early
        // assignment, so the called MS stays on the control channel and answers there.
        // The traffic channel is assigned later in the D-CONNECT ACKNOWLEDGE.
        let d_setup = DSetup {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: hook_on_off,
            simplex_duplex_selection: duplex,
            basic_service_information: pdu.basic_service_information.clone(),
            transmission_grant: called_grant,
            transmission_request_permission: false,
            call_priority: pdu.call_priority,
            notification_indicator: None,
            temporary_address: None,
            calling_party_address_ssi: Some(calling_ssi),
            calling_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };
        let (setup_sdu, _) = Self::build_d_setup_prim(&d_setup, called_usage, called_ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, None, called_addr, Layer2Service::Unacknowledged, None);
        queue.push_back(setup_msg);
    }

    /// Decode an external subscriber number Type3 element into a dial string. 4-bit BCD nibbles,
    /// most significant first (ETSI Table 14.59). ETSI caps it at 24 digits, so excess is dropped.
    fn decode_external_subscriber_number(field: &tetra_core::typed_pdu_fields::Type3FieldGeneric) -> String {
        const MAX_DIGITS: usize = 24;
        let nibble_count = (field.len / 4).min(MAX_DIGITS);
        if field.len / 4 > MAX_DIGITS {
            tracing::warn!(
                "external subscriber number {} digits exceeds ETSI max 24, truncating",
                field.len / 4
            );
        }
        let mut digits = String::with_capacity(nibble_count);
        for i in 0..nibble_count {
            let byte = field.raw.get(i / 2).copied().unwrap_or(0);
            let nibble = if i % 2 == 0 { byte >> 4 } else { byte & 0xf };
            match nibble {
                0..=9 => digits.push(char::from(b'0' + nibble)),
                0x0a => digits.push('*'),
                0x0b => digits.push('#'),
                0x0c => digits.push('+'),
                _ => {}
            }
        }
        digits
    }

    /// Set up an individual call whose far party is reached over Brew (off-cell ISSI or
    /// PBX/phone number). One traffic channel is opened for the local caller with network
    /// downlink media, and a SETUP REQUEST is sent to the backend. The caller is through
    /// connected later when the backend sends a CONNECT REQUEST.
    #[allow(clippy::too_many_arguments)]
    fn setup_individual_call_over_brew(
        &mut self,
        queue: &mut MessageQueue,
        message: &SapMsg,
        pdu: &USetup,
        calling_party: TetraAddress,
        duplex: bool,
        handle: u32,
        link_id: u32,
        endpoint_id: u32,
    ) {
        let calling_ssi = calling_party.ssi;
        if self.issi_in_individual_call(calling_ssi) {
            tracing::warn!("calling ISSI {} already in a call, rejecting", calling_ssi);
            self.reject_individual_setup(queue, message, DisconnectCause::ConcurrentSetUpNotSupported);
            return;
        }
        let called_ssi = pdu.called_party_ssi.map(|s| s as u32).unwrap_or(0);
        let number = pdu
            .external_subscriber_number
            .as_ref()
            .map(Self::decode_external_subscriber_number)
            .unwrap_or_default();

        let brew_uuid = uuid::Uuid::new_v4();

        // Over Brew there is no second local channel: the far leg lives in the backend, so a
        // duplex call still uses a single slot. `is_duplex` records the negotiated mode.
        let Some(call_id) = self.circuits.allocate_circuit(CircuitRequest {
            caller: calling_ssi,
            callee: called_ssi,
            comm_type: pdu.basic_service_information.communication_type,
            state: CircuitState::Setup(0),
            is_duplex: duplex,
            second_channel: false,
            over_brew: true,
            hook_on_off: pdu.hook_method_selection,
            is_local_origin: true,
            is_mobile_terminated: false,
            origin_brew_uuid: Some(brew_uuid),
            caller_route: MleRoute::new(handle, link_id, endpoint_id),
            // Duplex grants both. Simplex over Brew: the backend drives the floor with
            // SIMPLEX GRANTED/IDLE, so start with nobody holding it.
            floor: None,
        }) else {
            tracing::error!("Failed to allocate circuit for over-Brew U-SETUP: no free timeslot");
            self.reject_individual_setup(queue, message, DisconnectCause::CongestionInInfrastructure);
            return;
        };
        let circuit = self.circuits.get_circuit_by_callid(call_id).expect("just allocated");
        let ts = circuit.dl_ts();

        tracing::info!(
            "individual call over Brew: ISSI {} to dest={} number='{}' ts={} call_id={} duplex={} uuid={}",
            calling_ssi,
            called_ssi,
            number,
            ts,
            call_id,
            duplex,
            brew_uuid
        );

        // Open the traffic channel now with network downlink media so the local loopback is
        // suppressed. Audio comes from the backend, the caller's uplink goes to the backend.
        Self::signal_umac_circuit_open(queue, ts, circuit.usage_id, None, CircuitDlMediaSource::Network);

        self.send_d_call_proceeding(queue, message, pdu, call_id);

        let call = NetworkCallRequest {
            source: calling_ssi,
            destination: called_ssi,
            number,
            priority: pdu.call_priority,
            // ETSI Table 14.79 speech service, 14.52 circuit mode, 14.54 communication type.
            // An individual call is point-to-point (14.54 = 0); ETSI 14.5.3.1 mandates it.
            service: pdu.basic_service_information.speech_service.unwrap_or(0),
            mode: pdu.basic_service_information.circuit_mode_type.into_raw() as u8,
            duplex: duplex as u8,
            method: pdu.hook_method_selection as u8,
            communication: pdu.basic_service_information.communication_type.into_raw() as u8,
            grant: 0,
            // ETSI Table 14.81: 0 = allowed to request transmission.
            permission: 0,
            timeout: CallTimeout::T5m.into_raw() as u8,
            ownership: 1,
            queued: 0,
        };
        self.put_network_request(brew_uuid, call);
        Self::signal_brew(queue, CmceEvent::SetupRequest { call_id });
    }

    /// Backend is alerting (ringing) on an over-Brew call. Relay D-ALERT to the caller.
    fn rx_network_circuit_alert(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some(call) = self.live_individual_circuit(call_id) else {
            return;
        };
        if call.is_setup() {
            self.circuits.set_state(call_id, CircuitState::Alerting(0));
        }
        let d_alert = DAlert {
            call_identifier: call.call_id,
            call_time_out_set_up_phase: 0,
            reserved: false,
            simplex_duplex_selection: call.is_duplex,
            call_queued: false,
            basic_service_information: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut sdu = BitBuffer::new_autoexpand(20);
        d_alert.to_bitbuf(&mut sdu).expect("Failed to serialize DAlert");
        sdu.seek(0);
        self.send_to_caller(queue, &call, sdu, None);
    }

    /// Drive the local caller's floor on a simplex over-Brew call from backend SIMPLEX state.
    /// caller_talks true grants the caller transmit, false puts it in receive (the backend
    /// holds the floor). Only the local caller is on air, and the slot stays in traffic so the
    /// backend downlink keeps playing either way.
    fn brew_simplex_floor(&mut self, queue: &mut MessageQueue, call_id: u16, caller_talks: bool) {
        let Some(call) = self.live_individual_circuit(call_id) else {
            return;
        };
        if call.is_duplex {
            return; // Duplex has no floor cycle.
        }
        let (local, ts) = call.local_leg();
        let grant = if caller_talks {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };
        self.circuits.set_floor(call_id, if caller_talks { Some(local.ssi) } else { None });
        self.send_individual_tx_granted(queue, call_id, local.ssi, local, grant, ts);
    }

    /// Backend connected an over-Brew call. Through-connect the local caller: D-CONNECT with
    /// the channel allocation, then tell Brew media is ready and confirm the connect.
    fn rx_network_circuit_connect_request(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some(call) = self.live_individual_circuit(call_id) else {
            return;
        };
        if call.is_tx() {
            return;
        }
        self.circuits.set_state(call_id, CircuitState::Tx(0));

        let ts = call.dl_ts();
        let mut timeslots = [false; 4];
        timeslots[ts as usize - 1] = true;
        let chan_alloc = CmceChanAllocReq {
            usage: Some(call.usage_id),
            alloc_type: ChanAllocType::Replace,
            carrier: None,
            timeslots,
            ul_dl_assigned: UlDlAssignment::Both,
        };
        let d_connect = DConnect {
            call_identifier: call.call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: call.hook_on_off,
            simplex_duplex_selection: call.is_duplex,
            transmission_grant: TransmissionGrant::Granted,
            transmission_request_permission: false,
            call_ownership: true,
            call_priority: None,
            basic_service_information: None,
            temporary_address: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut connect_sdu = BitBuffer::new_autoexpand(30);
        d_connect.to_bitbuf(&mut connect_sdu).expect("Failed to serialize DConnect");
        connect_sdu.seek(0);
        self.send_to_caller(queue, &call, connect_sdu, Some(chan_alloc));

        // The traffic slot now carries this call's media, so confirm the connect to Brew.
        Self::signal_brew(queue, CmceEvent::ConnectConfirm { call_id });
        tracing::info!("individual call over Brew call_id={} active", call.call_id);
    }

    /// Reject a mobile-terminated over-Brew setup back to the backend. No circuit was created
    /// for it, so the rejection names the Brew session the backend offered it under.
    fn reject_network_circuit(&self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid, cause: DisconnectCause) {
        Self::signal_brew(
            queue,
            CmceEvent::SetupReject {
                brew_uuid,
                cause: cause as u8,
            },
        );
    }

    /// Mobile-terminated individual call from the backend (PBX/phone or off-cell ISSI) to a
    /// local MS. ETSI EN 300 392-2 clause 14.5.1. Accept toward the backend, then D-SETUP the
    /// called MS. The MS answers with U-ALERT/U-CONNECT, relayed back over Brew.
    fn rx_network_circuit_setup_terminated(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid) {
        let Some(call) = self.take_network_request(brew_uuid) else {
            tracing::warn!("terminated circuit setup uuid={} without call parameters", brew_uuid);
            return;
        };
        let dest = call.destination;
        if !self.is_individual_registered(dest) {
            tracing::warn!("terminated circuit setup for non-local ISSI {}, rejecting uuid={}", dest, brew_uuid);
            self.reject_network_circuit(queue, brew_uuid, DisconnectCause::CalledPartyNotReachable);
            return;
        }
        if self.issi_in_individual_call(dest) {
            tracing::warn!("terminated circuit setup for busy ISSI {}, rejecting", dest);
            self.reject_network_circuit(queue, brew_uuid, DisconnectCause::CalledPartyBusy);
            return;
        }

        let duplex = call.duplex != 0;
        let hook = call.method != 0;
        // Over Brew the far leg is the backend, so a single local slot carries the call.
        let Some(call_id) = self.circuits.allocate_circuit(CircuitRequest {
            caller: call.source,
            callee: dest,
            comm_type: CommunicationType::P2p,
            state: CircuitState::Setup(0),
            is_duplex: duplex,
            second_channel: false,
            over_brew: true,
            hook_on_off: hook,
            is_local_origin: false,
            is_mobile_terminated: true,
            origin_brew_uuid: Some(brew_uuid),
            // Backend caller has no local LLC link.
            caller_route: MleRoute::default(),
            floor: None,
        }) else {
            tracing::error!("Failed to allocate circuit for terminated over-Brew call: no free timeslot");
            self.reject_network_circuit(queue, brew_uuid, DisconnectCause::CongestionInInfrastructure);
            return;
        };
        let circuit = self.circuits.get_circuit_by_callid(call_id).expect("just allocated");
        let ts = circuit.dl_ts();

        let called_addr = TetraAddress::new(dest, SsiType::Issi);
        tracing::info!(
            "terminated individual call over Brew: caller {} to local ISSI {} ts={} call_id={} duplex={} uuid={}",
            call.source,
            dest,
            ts,
            call_id,
            duplex,
            brew_uuid
        );

        // Downlink audio from the backend, MS uplink forwarded to it.
        Self::signal_umac_circuit_open(queue, ts, circuit.usage_id, None, CircuitDlMediaSource::Network);

        Self::signal_brew(queue, CmceEvent::SetupAccept { call_id });

        // D-SETUP on the control channel. No early assignment: the MS moves to traffic on the
        // D-CONNECT ACKNOWLEDGE (ETSI Table 14.1, late assignment). Simplex lets the MS request
        // the floor, duplex grants it outright.
        let d_setup = DSetup {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: hook,
            simplex_duplex_selection: duplex,
            basic_service_information: BasicServiceInformation {
                circuit_mode_type: CircuitModeType::TchS,
                encryption_flag: false,
                communication_type: CommunicationType::P2p,
                slots_per_frame: None,
                speech_service: Some(0),
            },
            transmission_grant: if duplex {
                TransmissionGrant::Granted
            } else {
                TransmissionGrant::GrantedToOtherUser
            },
            transmission_request_permission: !duplex,
            call_priority: call.priority,
            notification_indicator: None,
            temporary_address: None,
            calling_party_address_ssi: Some(call.source),
            calling_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };
        let (setup_sdu, _) = Self::build_d_setup_prim(&d_setup, circuit.usage_id, ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, None, called_addr, Layer2Service::Unacknowledged, None);
        queue.push_back(setup_msg);
    }

    /// Reject an individual U-SETUP with a D-RELEASE to the caller (ETSI 14.5.1.3.2).
    fn reject_individual_setup(&mut self, queue: &mut MessageQueue, message: &SapMsg, cause: DisconnectCause) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {
            panic!()
        };
        let calling_addr = prim.received_tetra_address;
        let d_release = DRelease {
            call_identifier: 0, // no call identifier assigned yet, dummy reference
            disconnect_cause: cause,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut sdu = BitBuffer::new_autoexpand(20);
        d_release.to_bitbuf(&mut sdu).expect("Failed to serialize DRelease");
        sdu.seek(0);
        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: prim.handle,
                endpoint_id: prim.endpoint_id,
                link_id: prim.link_id,
                layer2service: Layer2Service::Unacknowledged,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None,
                main_address: calling_addr,
                tx_reporter: None,
            }),
        });
    }

    /// U-ALERT: the called party is ringing (on/off-hook only). Relay a D-ALERT to the
    /// caller so it can ring back. ETSI 14.5.1.1.1, 14.5.1.1.2.
    fn rx_u_alert(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        let pdu = match UAlert::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => pdu,
            Err(e) => {
                tracing::warn!("Failed parsing U-ALERT: {:?}", e);
                return;
            }
        };
        let Some(call) = self.live_individual_circuit(pdu.call_identifier) else {
            tracing::warn!("U-ALERT for unknown individual call_id={}", pdu.call_identifier);
            return;
        };
        if !call.is_setup() || !call.hook_on_off {
            tracing::warn!("U-ALERT ignored for call_id={} in state {:?}", call.call_id, call.state);
            return;
        }
        self.circuits.set_state(pdu.call_identifier, CircuitState::Alerting(0));

        // Terminated call: the on-air party is the called MS, so relay alerting to the backend.
        if call.is_mobile_terminated {
            Self::signal_brew(queue, CmceEvent::Alert { call_id: call.call_id });
            return;
        }

        // ETSI 14.5.1.1.1: the called party offers simplex when it cannot do the duplex call.
        if !pdu.simplex_duplex_selection {
            self.downgrade_individual_to_simplex(queue, pdu.call_identifier);
        }
        let call = self.live_individual_circuit(pdu.call_identifier).unwrap();

        // D-ALERT to caller. The old hook field is now Reserved and shall be 1 (ETSI Table 14.4).
        let d_alert = DAlert {
            call_identifier: call.call_id,
            call_time_out_set_up_phase: CallTimeoutSetupPhase::T60s as u8,
            reserved: true,
            simplex_duplex_selection: call.is_duplex,
            call_queued: false,
            basic_service_information: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut sdu = BitBuffer::new_autoexpand(20);
        d_alert.to_bitbuf(&mut sdu).expect("Failed to serialize DAlert");
        sdu.seek(0);
        self.send_to_caller(queue, &call, sdu, None);
    }

    /// U-CONNECT: the called party answered. Through-connect both legs. ETSI 14.5.1.1.
    fn rx_u_connect(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        let pdu = match UConnect::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => pdu,
            Err(e) => {
                tracing::warn!("Failed parsing U-CONNECT: {:?}", e);
                return;
            }
        };
        // ETSI 14.5.1.1.1: the called party offers simplex when it cannot do the duplex call.
        if !pdu.simplex_duplex_selection {
            self.downgrade_individual_to_simplex(queue, pdu.call_identifier);
        }
        let Some(call) = self.live_individual_circuit(pdu.call_identifier) else {
            tracing::warn!("U-CONNECT for unknown individual call_id={}", pdu.call_identifier);
            return;
        };
        if call.is_tx() {
            tracing::warn!("U-CONNECT for already-active call_id={}, ignoring", pdu.call_identifier);
            return;
        }
        self.circuits.set_state(pdu.call_identifier, CircuitState::Tx(0));

        // Terminated call: through-connect the MS and relay the answer to the backend. The MS
        // may have offered simplex if it could not do the requested duplex (ETSI 14.5.1.1.1).
        if call.is_mobile_terminated {
            self.circuits.update_circuit(pdu.call_identifier, |c| {
                c.is_duplex &= pdu.simplex_duplex_selection;
            });
            let call = self.live_individual_circuit(pdu.call_identifier).unwrap();
            self.connect_terminated(queue, &call);
            return;
        }
        let call = self.live_individual_circuit(pdu.call_identifier).unwrap();

        let caller_has_floor = call.floor == Some(call.caller);

        // Per-party channel allocation. Simplex shares one slot (called_ts == ts), duplex
        // gives each party its own. Duplex grants both parties (talk and receive); simplex
        // grants the floor holder and tells the other it is for another user.
        let make_chan_alloc = |ts: u8, usage: u8| {
            let mut timeslots = [false; 4];
            timeslots[ts as usize - 1] = true;
            CmceChanAllocReq {
                usage: Some(usage),
                alloc_type: ChanAllocType::Replace,
                carrier: None,
                timeslots,
                ul_dl_assigned: UlDlAssignment::Both,
            }
        };
        let caller_grant = if call.is_duplex || caller_has_floor {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };
        let called_grant = if call.is_duplex || !caller_has_floor {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };

        // D-CONNECT to the caller with its channel allocation. The caller owns the call.
        let d_connect = DConnect {
            call_identifier: call.call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: call.hook_on_off,
            simplex_duplex_selection: call.is_duplex,
            transmission_grant: caller_grant,
            transmission_request_permission: false,
            call_ownership: true,
            call_priority: None,
            basic_service_information: None,
            temporary_address: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut connect_sdu = BitBuffer::new_autoexpand(30);
        d_connect.to_bitbuf(&mut connect_sdu).expect("Failed to serialize DConnect");
        connect_sdu.seek(0);
        self.send_to_caller(queue, &call, connect_sdu, Some(make_chan_alloc(call.ul_ts(), call.usage_id)));

        // D-CONNECT ACKNOWLEDGE to the called party on the control channel, carrying its
        // channel allocation. This is the PDU that moves the called MS to the traffic
        // channel and switches its U-plane on (ETSI 14.5.1.4.1, late assignment), so it
        // needs the allocation to know which channel to render.
        let d_connect_ack = DConnectAcknowledge {
            call_identifier: call.call_id,
            call_time_out: CallTimeout::T5m as u8,
            transmission_grant: called_grant as u8,
            transmission_request_permission: false,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut ack_sdu = BitBuffer::new_autoexpand(20);
        d_connect_ack
            .to_bitbuf(&mut ack_sdu)
            .expect("Failed to serialize DConnectAcknowledge");
        ack_sdu.seek(0);
        queue.push_back(Self::build_sapmsg(
            ack_sdu,
            Some(make_chan_alloc(call.callee_ts(), call.callee_usage_id())),
            call.callee_addr(),
            Layer2Service::Unacknowledged,
            None,
        ));

        // Put the timeslot in traffic mode for the initial floor holder so its uplink
        // voice is looped to the peer on the downlink.
        if let Some(holder) = call.floor {
            let peer = if holder == call.caller { call.callee } else { call.caller };
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id: call.call_id,
                    source_issi: holder,
                    dest_gssi: peer,
                    ts: call.ul_ts(),
                }),
            });
        }

        tracing::info!("individual call_id={} active", call.call_id);
    }

    /// Through-connect a terminated over-Brew call once the MS answers: D-CONNECT ACKNOWLEDGE
    /// moves it to traffic, then notify the backend that media is live and the call connected.
    fn connect_terminated(&self, queue: &mut MessageQueue, call: &TetraCircuit) {
        let called_ts = call.callee_ts();
        let mut timeslots = [false; 4];
        timeslots[called_ts as usize - 1] = true;
        let chan_alloc = CmceChanAllocReq {
            usage: Some(call.callee_usage_id()),
            alloc_type: ChanAllocType::Replace,
            carrier: None,
            timeslots,
            ul_dl_assigned: UlDlAssignment::Both,
        };
        // Duplex grants the MS outright, simplex lets the backend drive the floor.
        let grant = if call.is_duplex {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };
        let d_connect_ack = DConnectAcknowledge {
            call_identifier: call.call_id,
            call_time_out: CallTimeout::T5m as u8,
            transmission_grant: grant as u8,
            transmission_request_permission: !call.is_duplex,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        let mut ack_sdu = BitBuffer::new_autoexpand(20);
        d_connect_ack
            .to_bitbuf(&mut ack_sdu)
            .expect("Failed to serialize DConnectAcknowledge");
        ack_sdu.seek(0);
        queue.push_back(Self::build_sapmsg(
            ack_sdu,
            Some(chan_alloc),
            call.callee_addr(),
            Layer2Service::Unacknowledged,
            None,
        ));

        let brew_uuid = call.brew_uuid().or(call.brew_origin_uuid).expect("terminated call has a Brew uuid");
        // Brew CONNECT_REQUEST, receiver to origin. Individual voice call is always ACELP P2P.
        let answer = NetworkCallRequest {
            source: call.caller,
            destination: call.callee,
            number: String::new(),
            priority: 0,
            service: 0,
            mode: 0,
            duplex: call.is_duplex as u8,
            method: call.hook_on_off as u8,
            communication: 0,
            grant: TransmissionGrant::Granted.into_raw() as u8,
            permission: 0,
            timeout: CallTimeout::T5m.into_raw() as u8,
            ownership: 1,
            queued: 0,
        };
        self.put_network_request(brew_uuid, answer);
        Self::signal_brew(queue, CmceEvent::ConnectRequest { call_id: call.call_id });
        tracing::info!("terminated individual call_id={} active", call.call_id);
    }

    /// ETSI 14.5.1.1.1: a called MS that cannot do the requested duplex call offers simplex in
    /// its U-ALERT or U-CONNECT. Honor it by collapsing the call onto the caller's slot, closing
    /// the second traffic channel, and dropping the duplex cross-route so it runs as simplex.
    fn downgrade_individual_to_simplex(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some(call) = self.live_individual_circuit(call_id) else {
            return;
        };
        let caller_ts = call.ul_ts();
        let caller_usage = call.usage_id;
        let Some(second_ts) = self.circuits.downgrade_duplex_to_simplex(call_id) else {
            return;
        };
        // Hook call: the answering called party transmits first (ETSI 14.5.1.2.1 a).
        self.circuits.set_floor(call_id, Some(call.callee));

        Self::signal_umac_circuit_close(queue, second_ts);
        self.circuits.release_timeslot(second_ts);

        // Re-open the caller slot as a shared simplex channel without the duplex peer route.
        Self::signal_umac_circuit_open(queue, caller_ts, caller_usage, None, CircuitDlMediaSource::LocalLoopback);
        tracing::info!("individual call_id={} downgraded to simplex, called offered simplex", call_id);
    }

    /// Release an individual call, notifying Brew if the call was over Brew.
    fn release_individual_call(&mut self, queue: &mut MessageQueue, call_id: u16, cause: DisconnectCause) {
        self.release_individual_call_inner(queue, call_id, cause, true);
    }

    /// Release an individual call: D-RELEASE to the local parties, then defer the circuit
    /// teardown so the stolen D-RELEASE transmits (same as the group path). For an over-Brew
    /// call the called leg is the backend, so it gets no D-RELEASE; instead Brew is notified
    /// when notify_brew is set (false when the release originated from Brew).
    fn release_individual_call_inner(&mut self, queue: &mut MessageQueue, call_id: u16, cause: DisconnectCause, notify_brew: bool) {
        let Some(call) = self.live_individual_circuit(call_id) else {
            return;
        };
        // Once active both parties are on the traffic channel, so steal the D-RELEASE onto
        // it. During setup or alerting they are still on the control channel, so send it
        // there, otherwise a reject or caller cancel never reaches the other party.
        let on_traffic = call.is_tx();
        // Each local party is on its own slot (the same slot for simplex). An over-Brew call
        // has one local leg. The backend gets no on-air release.
        let legs = if call.is_over_brew() {
            vec![call.local_leg()]
        } else {
            vec![(call.caller_addr(), call.ul_ts()), (call.callee_addr(), call.callee_ts())]
        };
        for (addr, party_ts) in legs {
            let d_release = DRelease {
                call_identifier: call_id,
                disconnect_cause: cause,
                notification_indicator: None,
                facility: None,
                proprietary: None,
            };
            let mut sdu = BitBuffer::new_autoexpand(20);
            d_release.to_bitbuf(&mut sdu).expect("Failed to serialize DRelease");
            sdu.seek(0);
            let msg = if on_traffic {
                Self::build_sapmsg_stealing(sdu, addr, party_ts)
            } else {
                Self::build_sapmsg(sdu, None, addr, Layer2Service::Unacknowledged, None)
            };
            queue.push_back(msg);
        }

        if call.is_over_brew() && notify_brew {
            Self::signal_brew(
                queue,
                CmceEvent::Release {
                    call_id,
                    cause: cause as u8,
                },
            );
        }

        // Defer teardown so the stolen D-RELEASE goes out. finalize_release only emits Brew
        // group notifications for group circuits, so an individual call stays inert there.
        self.circuits.set_state(call_id, CircuitState::Releasing(0));
    }

    /// Release individual calls that pass their setup/no-answer or call-length timeout.
    fn process_individual_timeouts(&mut self, queue: &mut MessageQueue) {
        // ETSI 14.6: T303 calling set-up timer 60 s, T310 call length min 30 s.
        // Values in timeslots, since TdmaTime ages in timeslots (~14 ms each).
        const SETUP_TIMEOUT_TS: i32 = 4235; // ~60 s
        const ACTIVE_TIMEOUT_TS: i32 = 21176; // ~300 s

        let now = self.dltime;
        let expired: Vec<u16> = self
            .circuits
            .find_circuits(|c| {
                if !c.is_individual_call() || c.is_releasing() {
                    return false;
                }
                let limit = if c.is_tx() { ACTIVE_TIMEOUT_TS } else { SETUP_TIMEOUT_TS };
                c.t_state.age(now) >= limit
            })
            .into_iter()
            .map(|c| c.call_id)
            .collect();
        for id in expired {
            tracing::info!("individual call_id={} timed out, releasing", id);
            self.release_individual_call(queue, id, DisconnectCause::ExpiryOfTimer);
        }
    }

    /// Floor holder of a simplex individual call released. Send D-TX CEASED to the on-air
    /// parties and put the timeslot into hangtime. ETSI 14.5.1.2. An over-Brew call has only
    /// the local caller on air, so the backend leg gets no D-TX CEASED.
    fn individual_tx_ceased(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some(call) = self.live_individual_circuit(call_id) else {
            return;
        };
        let ts = call.ul_ts();
        let addrs: Vec<TetraAddress> = if call.is_over_brew() {
            vec![call.local_leg().0]
        } else {
            vec![call.caller_addr(), call.callee_addr()]
        };
        self.circuits.set_floor(call_id, None);

        for addr in addrs {
            let d_tx_ceased = DTxCeased {
                call_identifier: call_id,
                transmission_request_permission: false,
                notification_indicator: None,
                facility: None,
                dm_ms_address: None,
                proprietary: None,
            };
            let mut sdu = BitBuffer::new_autoexpand(25);
            d_tx_ceased.to_bitbuf(&mut sdu).expect("Failed to serialize DTxCeased");
            sdu.seek(0);
            queue.push_back(Self::build_sapmsg_stealing(sdu, addr, ts));
        }

        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
        });
        tracing::info!("individual call_id={} floor released", call_id);
    }

    /// A party of a simplex individual call requests the floor. Grant it if free, send
    /// D-TX GRANTED to the requester and the peer, and resume traffic. ETSI 14.5.1.2.
    fn individual_tx_demand(&mut self, queue: &mut MessageQueue, call_id: u16, requester: u32) {
        let Some(circuit) = self.live_individual_circuit(call_id) else {
            return;
        };
        if requester != circuit.caller && requester != circuit.callee {
            tracing::warn!("U-TX DEMAND from non-party ISSI {} on call_id={}", requester, call_id);
            return;
        }
        // Wait for the current talker to cease before granting (ETSI 14.5.1.2.1 a).
        if let Some(holder) = circuit.floor {
            if holder != requester {
                tracing::warn!(
                    "U-TX DEMAND from ISSI {} rejected, ISSI {} holds the floor on call_id={}",
                    requester,
                    holder,
                    call_id
                );
                return;
            }
        }
        let ts = circuit.ul_ts();
        let over_brew = circuit.is_over_brew();
        let (calling, called) = (circuit.caller_addr(), circuit.callee_addr());
        let (requester_addr, peer) = if requester == calling.ssi {
            (calling, called)
        } else {
            (called, calling)
        };
        self.circuits.set_floor(call_id, Some(requester));

        self.send_individual_tx_granted(queue, call_id, requester, requester_addr, TransmissionGrant::Granted, ts);
        // The peer of an over-Brew call is the backend, which is off air and gets no D-TX GRANTED.
        if !over_brew {
            self.send_individual_tx_granted(queue, call_id, requester, peer, TransmissionGrant::GrantedToOtherUser, ts);
        }

        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                call_id,
                source_issi: requester,
                dest_gssi: peer.ssi,
                ts,
            }),
        });
        tracing::info!("individual call_id={} floor granted to ISSI {}", call_id, requester);
    }

    /// Send a D-TX GRANTED stolen onto the traffic channel to one party of an
    /// individual call, naming the current talker.
    fn send_individual_tx_granted(
        &self,
        queue: &mut MessageQueue,
        call_id: u16,
        talker_ssi: u32,
        target: TetraAddress,
        grant: TransmissionGrant,
        ts: u8,
    ) {
        let d_tx_granted = DTxGranted {
            call_identifier: call_id,
            transmission_grant: grant.into_raw() as u8,
            transmission_request_permission: false,
            encryption_control: false,
            reserved: false,
            notification_indicator: None,
            transmitting_party_type_identifier: Some(1), // SSI
            transmitting_party_address_ssi: Some(talker_ssi as u64),
            transmitting_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };
        let mut sdu = BitBuffer::new_autoexpand(50);
        d_tx_granted.to_bitbuf(&mut sdu).expect("Failed to serialize DTxGranted");
        sdu.seek(0);
        queue.push_back(Self::build_sapmsg_stealing(sdu, target, ts));
    }

    pub fn route_xx_deliver(&mut self, _queue: &mut MessageQueue, mut message: SapMsg) {
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

        // TODO FIXME: Besides these PDUs, we can also receive several signals (BUSY ind, CLOSE ind, etc)
        match pdu_type {
            CmcePduTypeUl::USetup => self.rx_u_setup(_queue, message),
            CmcePduTypeUl::UTxCeased => self.rx_u_tx_ceased(_queue, message),
            CmcePduTypeUl::UTxDemand => self.rx_u_tx_demand(_queue, message),
            CmcePduTypeUl::URelease => self.rx_u_release(_queue, message),
            CmcePduTypeUl::UDisconnect => self.rx_u_disconnect(_queue, message),
            CmcePduTypeUl::UAlert => self.rx_u_alert(_queue, message),
            CmcePduTypeUl::UConnect => self.rx_u_connect(_queue, message),
            CmcePduTypeUl::UInfo | CmcePduTypeUl::UStatus | CmcePduTypeUl::UCallRestore => {
                unimplemented_log!("{}", pdu_type);
            }
            _ => {
                panic!();
            }
        }
    }

    pub fn tick_start(&mut self, queue: &mut MessageQueue, dltime: TdmaTime) {
        self.dltime = dltime;

        // Check hangtime expiry for active local calls
        self.check_hangtime_expiry(queue);

        // Drive deferred D-RELEASE teardown
        self.process_releasing_calls(queue);

        // Release individual calls that pass their setup or call-length timeout
        self.process_individual_timeouts(queue);

        if let Some(tasks) = self.circuits.tick_start(dltime) {
            for task in tasks {
                match task {
                    CircuitMgrCmd::SendDSetup(call_id, usage, ts) => {
                        // Skip late-entry D-SETUP during hangtime. The traffic channel is still
                        // allocated and sending D-SETUP with NotGranted can prevent floor requests.
                        let Some(group_circuit) = self.live_group_circuit(call_id) else {
                            continue;
                        };
                        if group_circuit.is_tx_ceased() {
                            continue;
                        }

                        // Get our cached D-SETUP, build a prim and send it down the stack
                        let Some(cached) = self.circuits.get_setup_mut(call_id) else {
                            tracing::error!("No cached D-SETUP for call id {}", call_id);
                            continue;
                        };

                        // Throttle: if the previous D-SETUP hasn't reached a final state yet
                        // (still queued in UMAC), skip this re-send to avoid flooding the MCCH.
                        if let Some(r) = cached.receipt.as_ref() {
                            if !r.is_in_final_state() {
                                tracing::trace!(
                                    "Suppressing D-SETUP re-send for call_id={} (previous still {:?})",
                                    call_id,
                                    r.get_state()
                                );
                                continue;
                            }
                            if r.get_state() == TxState::Discarded {
                                tracing::debug!("Previous D-SETUP for call_id={} was discarded by UMAC, retrying", call_id);
                            }
                        }

                        // Update transmission_grant based on current call state:
                        // During hangtime (nobody transmitting), use NotGranted;
                        // during active TX, use GrantedToOtherUser.
                        cached.pdu.transmission_grant = if group_circuit.is_tx() {
                            TransmissionGrant::GrantedToOtherUser
                        } else {
                            TransmissionGrant::NotGranted
                        };
                        let dest_addr = cached.dest_addr;
                        let (sdu, chan_alloc) = Self::build_d_setup_prim(&cached.pdu, usage, ts, UlDlAssignment::Both);

                        // Create a fresh txreporter for this re-send
                        let reporter = TxReporter::new_unacked();

                        // Cache the reporter so we can check its state on the next tick and
                        // throttle if it is still pending in UMAC.
                        cached.receipt = Some(reporter.clone());

                        let prim = Self::build_sapmsg(sdu, Some(chan_alloc), dest_addr, Layer2Service::Unacknowledged, Some(reporter));
                        queue.push_back(prim);
                    }

                    CircuitMgrCmd::SendClose(call_id, ts) => {
                        tracing::warn!("need to send CLOSE for call id {}", call_id);
                        // Get our cached D-SETUP, build D-RELEASE and send
                        match self.circuits.get_setup(call_id) {
                            Some(cached) => {
                                let dest_addr = cached.dest_addr;
                                let sdu = Self::build_d_release_from_d_setup(&cached.pdu, DisconnectCause::ExpiryOfTimer);
                                let prim = Self::build_sapmsg(sdu, None, dest_addr, Layer2Service::Unacknowledged, None);
                                queue.push_back(prim);
                            }
                            None => tracing::error!("No cached D-SETUP for call id {}", call_id),
                        }

                        // Clean up call state, which also drops the cached setup and frees the slot
                        self.circuits.set_state(call_id, CircuitState::Releasing(0));
                        self.circuits.destroy_circuit(call_id);

                        // Signal UMAC to release the circuit
                        Self::signal_umac_circuit_close(queue, ts);
                    }
                }
            }
        }
    }

    /// Check if any active calls in hangtime have expired, and if so, release them
    fn check_hangtime_expiry(&mut self, queue: &mut MessageQueue) {
        // Hangtime: 5 multiframes = ~5 seconds
        const HANGTIME_FRAMES: i32 = 5 * 18 * 4;

        let now = self.dltime;
        let expired: Vec<u16> = self
            .circuits
            .find_circuits(|c| c.is_group_call() && c.is_tx_ceased() && c.t_state.age(now) > HANGTIME_FRAMES)
            .into_iter()
            .map(|c| c.call_id)
            .collect();

        for call_id in expired {
            tracing::info!("Hangtime expired for call_id={}, releasing", call_id);
            self.release_call(queue, call_id, DisconnectCause::ExpiryOfTimer);
        }
    }

    /// Release a group call. Moves it to Releasing immediately so it cannot be re-keyed or
    /// reused, steals one D-RELEASE onto the traffic channel, and leaves the teardown to
    /// process_releasing_calls. The circuit and timeslot stay allocated until then, which lets
    /// the stolen D-RELEASE transmit before the slot leaves traffic mode. With no cached
    /// D-SETUP there is no D-RELEASE to send, so it tears down at once.
    fn release_call(&mut self, queue: &mut MessageQueue, call_id: u16, disconnect_cause: DisconnectCause) {
        let Some(call) = self.live_group_circuit(call_id) else {
            return;
        };
        let ts = call.dl_ts();

        match self.circuits.take_setup(call_id) {
            Some(cached) => {
                let sdu = Self::build_d_release_from_d_setup(&cached.pdu, disconnect_cause);
                queue.push_back(Self::build_sapmsg_stealing(sdu, cached.dest_addr, ts));
                self.circuits.set_state(call_id, CircuitState::Releasing(0));
            }
            None => {
                tracing::warn!("No cached D-SETUP for call_id={}, cleaning up without D-RELEASE", call_id);
                self.circuits.set_state(call_id, CircuitState::Releasing(0));
                self.finalize_release(queue, call_id);
            }
        }
    }

    /// Close a releasing call's circuit once enough frames have passed since the D-RELEASE
    /// was stolen for it to transmit. Driven once per tick.
    fn process_releasing_calls(&mut self, queue: &mut MessageQueue) {
        // Two TDMA frames: the stolen D-RELEASE drains over the next frame while the slot
        // is still in traffic mode, then teardown one frame later.
        const CLOSE_AFTER_SEND_TS: i32 = 8;

        let now = self.dltime;
        let ready: Vec<u16> = self
            .circuits
            .find_circuits(|c| c.is_releasing() && c.t_state.age(now) >= CLOSE_AFTER_SEND_TS)
            .into_iter()
            .map(|c| c.call_id)
            .collect();

        for call_id in ready {
            self.finalize_release(queue, call_id);
        }
    }

    /// Tear down a released call: close the circuit(s), free the timeslot(s), notify Brew.
    fn finalize_release(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some(call) = self.circuits.get_circuit_by_callid(call_id) else {
            tracing::warn!("finalize_release for unknown call_id={}", call_id);
            return;
        };
        let ts = call.dl_ts();
        // Tell Brew the transmission is over, if a Brew session we opened is attached to this
        // call. A session opened for a local speaker has to be idled towards the backend; a
        // network-owned one is ended by the backend itself, so Brew only drops its local state
        // once the call is gone. An individual call has already been released towards Brew by
        // release_individual_call_inner.
        let closing_session = call.brew_uuid().filter(|_| call.is_group_call() && call.ul1_source.is_local());
        // The circuit is about to vanish, so hand the session over: the event below names the
        // call by id, and Brew can no longer resolve it through the circuit.
        if let Some(uuid) = closing_session {
            self.put_closing_session(call_id, uuid);
        }

        // A local duplex call also frees its second slot (over-Brew uses one slot).
        if let Some(peer_ts) = call.peer_ts() {
            Self::signal_umac_circuit_close(queue, peer_ts);
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts: peer_ts }),
            });
        }

        Self::signal_umac_circuit_close(queue, ts);

        // Ensure UMAC clears hangtime even if the CMCE circuit was already closed above.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts }),
        });

        // The call is fully gone at this point: drop it from the global circuit view, which
        // also frees its timeslot(s) and any cached D-SETUP.
        self.circuits.destroy_circuit(call_id);

        if closing_session.is_some() {
            Self::signal_brew(queue, CmceEvent::TxEnd { call_id });
        }
    }

    fn feature_check_u_setup(pdu: &USetup) -> bool {
        let mut supported = true;

        if !(pdu.area_selection == 0 || pdu.area_selection == 1) {
            unimplemented_log!("Area selection not supported: {}", pdu.area_selection);
            supported = false;
        };
        if pdu.hook_method_selection == true {
            unimplemented_log!("Hook method selection not supported: {}", pdu.hook_method_selection);
            supported = false;
        };
        if pdu.simplex_duplex_selection != false {
            unimplemented_log!("Only simplex calls supported: {}", pdu.simplex_duplex_selection);
            supported = false;
        };
        // if pdu.basic_service_information != 0xFC {
        //     // TODO FIXME implement parsing
        //     tracing::error!("Basic service information not supported: {}", pdu.basic_service_information);
        //     return;
        // };
        // request_to_transmit_send_data can be false for speech group calls — the MS
        // implicitly requests to transmit by initiating the call. No action needed.
        if pdu.clir_control != 0 {
            unimplemented_log!("clir_control not supported: {}", pdu.clir_control);
        };
        if pdu.called_party_ssi.is_none() || pdu.called_party_short_number_address.is_some() || pdu.called_party_extension.is_some() {
            unimplemented_log!("we only support ssi-based calling");
        };
        // Then, we warn about some other unhandled/unsupported fields
        if let Some(v) = &pdu.external_subscriber_number {
            unimplemented_log!("external_subscriber_number not supported: {:?}", v);
        };
        if let Some(v) = &pdu.facility {
            unimplemented_log!("facility not supported: {:?}", v);
        };
        if let Some(v) = &pdu.dm_ms_address {
            unimplemented_log!("dm_ms_address not supported: {:?}", v);
        };
        if let Some(v) = &pdu.proprietary {
            unimplemented_log!("proprietary not supported: {:?}", v);
        };

        supported
    }

    /// Handle U-TX CEASED: radio released PTT
    /// Response: send D-TX CEASED via FACCH to all group members, enter hangtime
    fn rx_u_tx_ceased(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match UTxCeased::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-TX CEASED: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;

        // Individual call: the floor holder released. Duplex has no floor (ETSI 14.5.1.2.1),
        // so it is ignored there.
        if self.is_individual_call_id(call_id) {
            if !self.individual_is_duplex(call_id) {
                self.individual_tx_ceased(queue, call_id);
            }
            return;
        }

        // Look up the group call in the global circuit view
        let Some(circuit) = self.live_group_circuit(call_id) else {
            tracing::warn!("U-TX CEASED for unknown call_id={}", call_id);
            return;
        };

        // Check if already in hangtime - ignore duplicate U-TX CEASED to avoid resetting timer
        if circuit.is_tx_ceased() {
            tracing::debug!("U-TX CEASED: already in hangtime for call_id={}, ignoring duplicate", call_id);
            return;
        }

        tracing::info!("U-TX CEASED: PTT released on call_id={}, entering hangtime", call_id);

        let ts = circuit.dl_ts();
        self.circuits.set_state(call_id, CircuitState::TxCeased(0));

        // Get dest address from cached setup
        let Some(cached) = self.circuits.get_setup(call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = cached.dest_addr;

        // Send D-TX CEASED via FACCH (stealing) to all group members
        let d_tx_ceased = DTxCeased {
            call_identifier: call_id,
            transmission_request_permission: false, // ETSI 14.8.43: 0 = allowed to request transmission
            notification_indicator: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(25);
        d_tx_ceased.to_bitbuf(&mut sdu).expect("Failed to serialize DTxCeased");
        sdu.seek(0);
        tracing::info!("-> {:?} sdu {}", d_tx_ceased, sdu.dump_bin());

        // Send via FACCH (stealing channel) so radios on the traffic channel hear the beep
        let msg = Self::build_sapmsg_stealing(sdu, dest_addr, ts);
        queue.push_back(msg);

        // Notify UMAC to enter hangtime signalling mode on this traffic timeslot.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
        });

        // Notify Brew to stop forwarding audio. Only a session Brew opened for this call has
        // a uuid on the circuit, so no further routing check is needed.
        if circuit.brew_uuid().is_some() {
            Self::signal_brew(queue, CmceEvent::TxEnd { call_id });
        }
    }

    /// Handle U-TX DEMAND: another radio requests floor during hangtime
    /// Response: send D-TX GRANTED via FACCH, resume voice path
    fn rx_u_tx_demand(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        let requesting_party = prim.received_tetra_address;

        let pdu = match UTxDemand::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-TX DEMAND: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;

        // Individual call: hand the floor to the requesting party. Duplex has no floor
        // (ETSI 14.5.1.2.1), so it is ignored there.
        if self.is_individual_call_id(call_id) {
            if !self.individual_is_duplex(call_id) {
                self.individual_tx_demand(queue, call_id, requesting_party.ssi);
            }
            return;
        }

        let Some(circuit) = self.live_group_circuit(call_id) else {
            tracing::warn!("U-TX DEMAND for unknown call_id={}", call_id);
            return;
        };

        tracing::info!("U-TX DEMAND: ISSI {} requests floor on call_id={}", requesting_party.ssi, call_id);

        // ETSI 14.5.2.2.1 b): if another MS is already transmitting, the SwMI should
        // normally wait for that party to finish before granting. Reject the request.
        if circuit.is_tx() {
            tracing::warn!(
                "U-TX DEMAND from ISSI {} rejected, ISSI {} already transmitting on call_id={}",
                requesting_party.ssi,
                circuit.floor.unwrap_or(0),
                call_id
            );
            return;
        }

        // Grant the floor to the requesting MS. A local-origin call also transfers ownership
        // to the new talker, matching the legacy caller_addr update.
        let ts = circuit.dl_ts();
        self.circuits.set_state(call_id, CircuitState::Tx(0));
        self.circuits.update_circuit(call_id, |c| {
            c.floor = Some(requesting_party.ssi);
            if c.is_local_origin {
                c.caller = requesting_party.ssi;
            }
        });

        let Some(cached) = self.circuits.get_setup(call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = cached.dest_addr;

        // ETSI 14.5.2.2.1 b): Send individual D-TX GRANTED (Granted) to requesting MS FIRST
        let d_tx_granted_individual = DTxGranted {
            call_identifier: call_id,
            transmission_grant: TransmissionGrant::Granted.into_raw() as u8,
            transmission_request_permission: false,
            encryption_control: false,
            reserved: false,
            notification_indicator: None,
            transmitting_party_type_identifier: Some(1), // SSI
            transmitting_party_address_ssi: Some(requesting_party.ssi as u64),
            transmitting_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(50);
        d_tx_granted_individual.to_bitbuf(&mut sdu).expect("Failed to serialize DTxGranted");
        sdu.seek(0);
        tracing::info!("-> {:?} sdu {}", d_tx_granted_individual, sdu.dump_bin());

        let requesting_addr = TetraAddress::new(requesting_party.ssi, SsiType::Issi);
        let msg = Self::build_sapmsg_stealing(sdu, requesting_addr, ts);
        queue.push_back(msg);

        // ETSI 14.5.2.2.1 b): Send group D-TX GRANTED (GrantedToOtherUser) to GSSI
        self.send_d_tx_granted_facch(queue, call_id, requesting_party.ssi, dest_addr.ssi, ts);

        // Notify UMAC to resume traffic mode (exit hangtime) for this timeslot.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                call_id,
                source_issi: requesting_party.ssi,
                dest_gssi: dest_addr.ssi,
                ts,
            }),
        });

        // Notify Brew of speaker change (local MS taking floor)
        if net_brew::is_brew_gssi_routable(&self.config, dest_addr.ssi) {
            Self::signal_brew(queue, CmceEvent::TxStart { call_id });
        }
    }

    /// Handle U-RELEASE: radio explicitly releases the call
    fn rx_u_release(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match URelease::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-RELEASE: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;
        tracing::info!("U-RELEASE: call_id={} cause={}", call_id, pdu.disconnect_cause);
        if self.is_individual_call_id(call_id) {
            self.release_individual_call(queue, call_id, DisconnectCause::UserRequestedDisconnection);
            return;
        }
        self.release_call(queue, call_id, DisconnectCause::UserRequestedDisconnection);
    }

    /// Handle U-DISCONNECT: MS requests call disconnection (ETSI 14.5.2.3.1)
    /// Call owner → release entire group call with D-RELEASE (cause=1)
    /// Non-call owner → reject with D-RELEASE cause=8 individually addressed to sender
    fn rx_u_disconnect(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };
        let sender = prim.received_tetra_address;
        let ul_handle = prim.handle;
        let ul_link_id = prim.link_id;
        let ul_endpoint_id = prim.endpoint_id;

        let pdu = match UDisconnect::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-DISCONNECT: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;
        let disconnect_cause = pdu.disconnect_cause;

        // Individual call: either party may disconnect (ETSI 14.5.1.3.1). The MS expects
        // a D-RELEASE in response, which release_individual_call sends to both legs.
        if self.is_individual_call_id(call_id) {
            tracing::info!("U-DISCONNECT: ISSI {} disconnecting individual call_id={}", sender.ssi, call_id);
            self.release_individual_call(queue, call_id, DisconnectCause::UserRequestedDisconnection);
            return;
        }

        let Some(circuit) = self.live_group_circuit(call_id) else {
            tracing::debug!("U-DISCONNECT for unknown call_id={} (likely duplicate)", call_id);
            return;
        };

        // `caller` carries the call owner. A network-initiated call has none and stores 0,
        // which can never match an on-air sender.
        let is_call_owner = circuit.caller == sender.ssi;

        if is_call_owner {
            // Call owner: tear down the entire group call
            tracing::info!("U-DISCONNECT: call owner ISSI {} disconnecting call_id={}", sender.ssi, call_id);
            self.release_call(queue, call_id, DisconnectCause::UserRequestedDisconnection);
        } else {
            // Non-call owner: reject with D-RELEASE cause=8 ("Requested service not available")
            // individually addressed back to the sender. The group call continues.
            tracing::info!(
                "U-DISCONNECT: non-call-owner ISSI {} rejected for call_id={} cause={}",
                sender.ssi,
                call_id,
                disconnect_cause
            );

            let d_release = DRelease {
                call_identifier: call_id,
                disconnect_cause: DisconnectCause::RequestedServiceNotAvailable,
                notification_indicator: None,
                facility: None,
                proprietary: None,
            };

            let mut sdu = BitBuffer::new_autoexpand(32);
            d_release.to_bitbuf(&mut sdu).expect("Failed to serialize DRelease");
            sdu.seek(0);
            tracing::info!("-> {:?} sdu {}", d_release, sdu.dump_bin());

            let sender_addr = TetraAddress::new(sender.ssi, SsiType::Issi);
            let msg = SapMsg {
                sap: Sap::LcmcSap,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Mle,
                msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                    sdu,
                    handle: ul_handle,
                    endpoint_id: ul_endpoint_id,
                    link_id: ul_link_id,
                    layer2service: Layer2Service::Unacknowledged,
                    pdu_prio: 0,
                    layer2_qos: 0,
                    stealing_permission: false,
                    stealing_repeats_flag: false,
                    chan_alloc: None,
                    main_address: sender_addr,
                    tx_reporter: None,
                }),
            };
            queue.push_back(msg);
        }
    }

    /// Handle CallControl messages from UMAC
    pub fn rx_call_control(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        let SapMsgInner::CmceCallControl(call_control) = message.msg else {
            panic!("Expected CmceCallControl message");
        };

        match call_control {
            CallControl::UlInactivityTimeout { ts } => {
                self.handle_ul_inactivity_timeout(queue, ts);
            }
            _ => {
                tracing::warn!("Unexpected CallControl message: {:?}", call_control);
            }
        }
    }

    /// Handle call events from Brew. The event only names the call, so anything else comes
    /// from the global circuit view or the network call request deposited alongside it.
    pub fn rx_brew_event(&mut self, queue: &mut MessageQueue, event: BrewEvent) {
        match event {
            BrewEvent::TxStart { brew_uuid } => {
                self.rx_network_tx_start(queue, brew_uuid);
            }
            BrewEvent::TxEnd { call_id } => {
                self.rx_network_call_end(queue, call_id);
            }
            BrewEvent::SetupRequest { brew_uuid } => {
                self.rx_network_circuit_setup_terminated(queue, brew_uuid);
            }
            BrewEvent::SetupAccept { call_id } => {
                tracing::debug!("over-Brew call setup accepted call_id={}", call_id);
            }
            BrewEvent::ConnectConfirm { call_id } => {
                tracing::debug!("terminated over-Brew call connect confirmed call_id={}", call_id);
            }
            BrewEvent::Alert { call_id } => {
                self.rx_network_circuit_alert(queue, call_id);
            }
            BrewEvent::ConnectRequest { call_id } => {
                self.rx_network_circuit_connect_request(queue, call_id);
            }
            BrewEvent::SetupReject { call_id, cause } | BrewEvent::Release { call_id, cause } => {
                let disconnect_cause = DisconnectCause::try_from(cause as u64).unwrap_or(DisconnectCause::CallRejectedByTheCalledParty);
                // The teardown came from Brew, so do not echo a release back to it.
                self.release_individual_call_inner(queue, call_id, disconnect_cause, false);
            }
            BrewEvent::SimplexGranted { call_id } => {
                // Far party (backend) holds the floor: the local caller switches to receive.
                self.brew_simplex_floor(queue, call_id, false);
            }
            BrewEvent::SimplexIdle { call_id } => {
                // Floor free: grant it to the local caller so it can talk.
                self.brew_simplex_floor(queue, call_id, true);
            }
        }
    }

    /// Handle a transmission started by the network: a new group call, or a new speaker on an
    /// existing one. Parameters were deposited by Brew alongside the signal.
    fn rx_network_tx_start(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid) {
        let Some(request) = self.take_network_request(brew_uuid) else {
            tracing::warn!("CMCE: network tx start uuid={} without call parameters", brew_uuid);
            return;
        };
        self.rx_network_call_start(queue, brew_uuid, request.source, request.destination, request.priority);
    }

    /// Handle network-initiated group call start
    fn rx_network_call_start(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid, source_issi: u32, dest_gssi: u32, _priority: u8) {
        assert!(net_brew::is_brew_gssi_routable(&self.config, dest_gssi));

        if !self.has_listener(dest_gssi) {
            tracing::info!(
                "CMCE: ignoring network call start uuid={} gssi={} (no listeners)",
                brew_uuid,
                dest_gssi
            );
            self.drop_group_calls_if_unlistened(queue, dest_gssi);
            // Nothing carries this session on air, so it never becomes a call. Brew drops the
            // session on its own once it sees no circuit appeared for it.
            return;
        }

        // Check if there is an active call for this GSSI (speaker change scenario)
        if let Some(circuit) = self.find_live_circuit(|c| c.is_group_call() && c.callee == dest_gssi) {
            // Reject speaker change if a local MS is already transmitting
            if circuit.is_tx() {
                tracing::warn!(
                    "CMCE: network speaker change rejected, ISSI {} already transmitting on gssi={}",
                    circuit.floor.unwrap_or(0),
                    dest_gssi
                );
                return;
            }

            let call_id_val = circuit.call_id;
            let ts = circuit.dl_ts();

            // Speaker change during hangtime
            tracing::info!(
                "CMCE: network call speaker change gssi={} new_speaker={} (was {})",
                dest_gssi,
                source_issi,
                circuit.floor.unwrap_or(0)
            );

            // The backend issues a fresh UUID for each speaker, so re-point the media routes
            // at the new session: downlink is local plus the Brew peer, uplink comes from it.
            self.circuits.set_state(call_id_val, CircuitState::Tx(0));
            self.circuits.update_circuit(call_id_val, |c| {
                c.floor = Some(source_issi);
                c.dl1_source = CircuitStreamDest::LocalAndRemote(Some(ts), Some(brew_uuid));
                c.ul1_source = CircuitStreamSrc::Remote(Some(brew_uuid));
                if !c.is_local_origin {
                    c.brew_origin_uuid = Some(brew_uuid);
                }
            });

            // Send D-TX GRANTED via FACCH to notify radios of new speaker
            self.send_d_tx_granted_facch(queue, call_id_val, source_issi, dest_gssi, ts);

            // Notify UMAC to resume traffic mode (exit hangtime) for this timeslot.
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id: call_id_val,
                    source_issi,
                    dest_gssi,
                    ts,
                }),
            });

            // Nothing to answer Brew: the circuit now carries the new session, which is all
            // Brew needs to play the backend audio out on this call.
            return;
        }

        // New network call: the backend holds the floor, so the media routes point at Brew.
        let Some(call_id) = self.circuits.allocate_circuit(CircuitRequest {
            caller: 0,
            callee: dest_gssi,
            comm_type: CommunicationType::P2Mp,
            state: CircuitState::Tx(0),
            is_duplex: false,
            second_channel: false,
            over_brew: true,
            hook_on_off: false,
            is_local_origin: false,
            is_mobile_terminated: false,
            origin_brew_uuid: Some(brew_uuid),
            caller_route: MleRoute::default(),
            floor: Some(source_issi),
        }) else {
            tracing::warn!("CMCE: failed to allocate circuit for network call: no free timeslot");
            return;
        };
        let circuit = self.circuits.get_circuit_by_callid(call_id).expect("just allocated");
        let ts = circuit.dl_ts();
        let usage = circuit.usage_id;
        self.circuits.update_circuit(call_id, |c| {
            c.ul1_source = CircuitStreamSrc::Remote(Some(brew_uuid));
        });

        tracing::info!(
            "CMCE: starting NEW network call brew_uuid={} gssi={} speaker={} ts={} call_id={}",
            brew_uuid,
            dest_gssi,
            source_issi,
            ts,
            call_id
        );

        // Signal UMAC to open DL and UL circuits
        Self::signal_umac_circuit_open(queue, ts, usage, None, CircuitDlMediaSource::LocalLoopback);

        tracing::debug!(
            "CMCE: sending D-SETUP for NEW call call_id={} gssi={} (network-initiated)",
            call_id,
            dest_gssi
        );

        // Send D-SETUP to group (broadcast on MCCH)
        let dest_addr = TetraAddress::new(dest_gssi, SsiType::Gssi);
        let d_setup = DSetup {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: false,
            simplex_duplex_selection: false, // Simplex
            basic_service_information: BasicServiceInformation {
                circuit_mode_type: CircuitModeType::TchS,
                encryption_flag: false,
                communication_type: CommunicationType::P2Mp,
                slots_per_frame: None,
                speech_service: Some(0),
            },
            transmission_grant: TransmissionGrant::GrantedToOtherUser,
            transmission_request_permission: false,
            call_priority: 0,
            notification_indicator: None,
            temporary_address: None,
            calling_party_address_ssi: Some(source_issi),
            calling_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        // Cache for late-entry re-sends and for building the D-RELEASE at teardown.
        self.circuits.cache_setup(call_id, d_setup, dest_addr);
        let d_setup_ref = &self.circuits.get_setup(call_id).unwrap().pdu;

        let (setup_sdu, setup_chan_alloc) = Self::build_d_setup_prim(d_setup_ref, usage, ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, Some(setup_chan_alloc), dest_addr, Layer2Service::Unacknowledged, None);
        queue.push_back(setup_msg);

        // Send D-CONNECT to group
        let d_connect = DConnect {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: false,
            simplex_duplex_selection: false, // Simplex
            transmission_grant: TransmissionGrant::GrantedToOtherUser,
            transmission_request_permission: false,
            call_ownership: false,
            call_priority: None,
            basic_service_information: None,
            temporary_address: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        let mut connect_sdu = BitBuffer::new_autoexpand(30);
        d_connect.to_bitbuf(&mut connect_sdu).expect("Failed to serialize DConnect");
        connect_sdu.seek(0);

        let connect_msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu: connect_sdu,
                handle: 0, // Broadcast to group, no specific handle
                endpoint_id: 0,
                link_id: 0,
                layer2service: Layer2Service::Unacknowledged,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None, // Already sent in D-SETUP
                main_address: dest_addr,
                tx_reporter: None,
            }),
        };
        queue.push_back(connect_msg);
    }

    /// Handle network call end: the backend transmission feeding this call stopped.
    fn rx_network_call_end(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some(circuit) = self.live_group_circuit(call_id) else {
            tracing::debug!("CMCE: network call end for unknown group call_id={}", call_id);
            return;
        };

        tracing::info!("CMCE: network call ended call_id={} gssi={}", call_id, circuit.callee);

        // If currently transmitting, enter hangtime instead of immediate release
        let tx_active = circuit.is_tx();
        let dest_gssi = circuit.callee;
        let ts = circuit.dl_ts();

        if tx_active {
            // The network speaker is gone: drop the Brew leg and put the slot back on local
            // media, so a local MS can take the floor during hangtime.
            self.circuits.set_state(call_id, CircuitState::TxCeased(0));
            self.circuits.update_circuit(call_id, |c| {
                c.dl1_source = CircuitStreamDest::Local(Some(ts));
                c.ul1_source = CircuitStreamSrc::Local(Some(ts));
            });
            // Send D-TX CEASED via FACCH
            self.send_d_tx_ceased_facch(queue, call_id, dest_gssi, ts);

            // Notify UMAC to enter hangtime signalling mode on this traffic timeslot.
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
            });
        } else {
            // Already in hangtime or idle, release immediately
            self.release_call(queue, call_id, DisconnectCause::SwmiRequestedDisconnection);
        }
    }

    /// Send D-TX GRANTED via FACCH stealing
    fn send_d_tx_granted_facch(&mut self, queue: &mut MessageQueue, call_id: u16, source_issi: u32, dest_gssi: u32, ts: u8) {
        let pdu = DTxGranted {
            call_identifier: call_id,
            transmission_grant: TransmissionGrant::GrantedToOtherUser.into_raw() as u8,
            transmission_request_permission: false,
            encryption_control: false,
            reserved: false,
            notification_indicator: None,
            transmitting_party_type_identifier: Some(1), // SSI
            transmitting_party_address_ssi: Some(source_issi as u64),
            transmitting_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(30);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DTxGranted");
        sdu.seek(0);
        tracing::info!("-> FACCH {:?} sdu {}", pdu, sdu.dump_bin());

        let dest_addr = TetraAddress::new(dest_gssi, SsiType::Gssi);
        let msg = Self::build_sapmsg_stealing(sdu, dest_addr, ts);
        queue.push_back(msg);
    }

    /// Handle UL inactivity timeout from UMAC: a radio disappeared mid-transmission.
    /// Treat identically to rx_u_tx_ceased — force TX ceased, enter hangtime.
    fn handle_ul_inactivity_timeout(&mut self, queue: &mut MessageQueue, ts: u8) {
        // Individual call: the floor holder went silent. Release the floor and enter hangtime.
        // Only for a connected call. During setup and alerting there is no floor on the air
        // yet, so an inactivity timeout there is the ringing delay, not a silent talker. Ceasing
        // then would clear the floor holder and leave the slot in hangtime at through-connect.
        if let Some(id) = self
            .find_live_circuit(|c| c.is_individual_call() && c.ul1_source.get_ts() == Some(ts) && c.is_tx() && c.floor.is_some())
            .map(|c| c.call_id)
        {
            tracing::warn!("UL inactivity timeout on ts={}, releasing floor for individual call_id={}", ts, id);
            self.individual_tx_ceased(queue, id);
            return;
        }

        // Find the group call on this timeslot that is transmitting
        let Some(circuit) = self.find_live_circuit(|c| c.is_group_call() && c.dl1_source.get_ts() == Some(ts) && c.is_tx()) else {
            tracing::debug!("UL inactivity timeout on ts={} but no active transmitting call found", ts);
            return;
        };
        let call_id = circuit.call_id;

        tracing::warn!("UL inactivity timeout on ts={}, forcing TX ceased for call_id={}", ts, call_id);

        let dest_gssi = circuit.callee;
        self.circuits.set_state(call_id, CircuitState::TxCeased(0));

        // Send D-TX CEASED via FACCH to all group members
        self.send_d_tx_ceased_facch(queue, call_id, dest_gssi, ts);

        // Notify UMAC to enter hangtime signalling mode
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
        });

        // Notify Brew to stop forwarding audio
        if circuit.brew_uuid().is_some() {
            Self::signal_brew(queue, CmceEvent::TxEnd { call_id });
        }
    }

    /// Send D-TX CEASED via FACCH stealing
    fn send_d_tx_ceased_facch(&mut self, queue: &mut MessageQueue, call_id: u16, dest_gssi: u32, ts: u8) {
        let pdu = DTxCeased {
            call_identifier: call_id,
            transmission_request_permission: false, // ETSI 14.8.43: 0 = allowed to request transmission
            notification_indicator: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(30);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DTxCeased");
        sdu.seek(0);
        tracing::info!("-> FACCH {:?} sdu {}", pdu, sdu.dump_bin());

        let dest_addr = TetraAddress::new(dest_gssi, SsiType::Gssi);
        let msg = Self::build_sapmsg_stealing(sdu, dest_addr, ts);
        queue.push_back(msg);
    }
}
