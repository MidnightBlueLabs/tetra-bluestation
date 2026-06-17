use std::collections::{HashMap, HashSet};

use tetra_config::bluestation::SharedConfig;
use tetra_core::{BitBuffer, Direction, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity, unimplemented_log};
use tetra_core::{Layer2Service, TimeslotOwner, TxReporter, TxState};
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
    structs::cmce_circuit::CmceCircuit,
};
use tetra_saps::{
    SapMsg, SapMsgInner,
    control::{
        brew::{BrewSubscriberAction, MmSubscriberUpdate},
        call_control::{CallControl, Circuit, CircuitDlMediaSource, NetworkCircuitCall},
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
    cmce::components::circuit_mgr::{CircuitMgr, CircuitMgrCmd},
};

/// Clause 11 Call Control CMCE sub-entity
pub struct CcBsSubentity {
    config: SharedConfig,
    dltime: TdmaTime,
    /// Cached D-SETUP PDUs for late-entry re-sends: call_id -> (D-SETUP PDU, dest address, tx reporter)
    cached_setups: HashMap<u16, (DSetup, TetraAddress, Option<TxReporter>)>,
    circuits: CircuitMgr,
    /// Active group calls: call_id -> call info
    active_calls: HashMap<u16, ActiveCall>,
    /// Registered subscriber groups (ISSI -> set of GSSIs)
    subscriber_groups: HashMap<u32, HashSet<u32>>,
    /// Listener counts per GSSI
    group_listeners: HashMap<u32, usize>,
    /// Calls whose D-RELEASE has been sent and whose circuit teardown is deferred a few
    /// frames so the stolen D-RELEASE transmits. These are no longer in active_calls.
    releasing_calls: Vec<ReleasingCall>,
    /// Active individual (point-to-point ISSI-to-ISSI) calls: call_id -> call info.
    /// Kept separate from active_calls (group) so group handling is untouched.
    individual_calls: HashMap<u16, IndividualCall>,
}

/// Origin of a group call
#[derive(Clone)]
enum CallOrigin {
    /// Local MS-initiated call, needs MLE routing for individual addressing
    Local {
        caller_addr: TetraAddress, // For D-CALL-PROCEEDING, D-CONNECT routing
    },
    /// Network-initiated call from TetraPack/Brew
    Network {
        brew_uuid: uuid::Uuid, // For Brew tracking
    },
}

/// A call being released. The call is removed from active_calls when this is created, so
/// it cannot be re-keyed or reused. D-RELEASE is stolen onto the traffic channel (it only
/// transmits while the slot is in traffic mode) at sent_at, then the circuit is closed a
/// couple frames later. Carries everything teardown needs, since the active_calls entry is
/// already gone.
struct ReleasingCall {
    call_id: u16,
    ts: u8,
    /// Second timeslot of a duplex individual call, freed alongside ts.
    peer_ts: Option<u8>,
    dest_gssi: u32,
    is_local: bool,
    brew_uuid: Option<uuid::Uuid>,
    sent_at: TdmaTime,
}

/// State of an individual (point-to-point) call. An unexpected PDU for the current
/// state is logged and ignored. ETSI EN 300 392-2 clause 14.5.1.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IndividualCallState {
    /// D-SETUP sent to the called party, waiting for U-ALERT (on/off-hook) or U-CONNECT.
    SetupSent,
    /// Called party alerted (on/off-hook only), waiting for U-CONNECT.
    Alerting,
    /// Through-connected, traffic channel open.
    Active,
}

/// An individual ISSI-to-ISSI call. ETSI EN 300 392-2 clause 14.5.1.
/// The called party is addressed by its ISSI. The calling party keeps the MLE routing
/// (handle, link_id, endpoint_id) from its U-SETUP so later PDUs reach it over the
/// established LLC link.
#[derive(Clone)]
struct IndividualCall {
    call_id: u16,
    calling_addr: TetraAddress,
    called_addr: TetraAddress,
    calling_handle: u32,
    calling_link_id: u32,
    calling_endpoint_id: u32,
    /// Caller traffic timeslot and its usage marker.
    ts: u8,
    usage: u8,
    /// Called party traffic timeslot and usage marker. Same as ts/usage for a simplex
    /// call (one shared slot). A duplex call uses a second slot so both parties transmit
    /// at once, and we cross-route each uplink to the other party's downlink.
    called_ts: u8,
    called_usage: u8,
    /// false = simplex (we control the floor), true = duplex (both granted, no floor).
    duplex: bool,
    /// true when the far party is reached over Brew (off-cell ISSI or PBX/phone number).
    /// The called leg is the backend, not a local MS, so it gets no on-air signalling and
    /// its downlink audio is fed from Brew instead of a local uplink loopback.
    over_brew: bool,
    /// Brew session UUID for an over-Brew call.
    brew_uuid: Option<uuid::Uuid>,
    /// true = on/off-hook signalling with alerting, false = direct through-connect.
    hook_on_off: bool,
    state: IndividualCallState,
    /// ISSI that currently holds the floor in a simplex call. None in duplex.
    floor_holder: Option<u32>,
    /// dltime the call entered its current phase, used for setup and no-answer timeouts.
    phase_started: TdmaTime,
}

/// Tracks an active group call (local or network-initiated)
#[derive(Clone)]
struct ActiveCall {
    origin: CallOrigin,
    dest_gssi: u32,   // Destination group
    source_issi: u32, // Current speaker
    ts: u8,
    usage: u8,
    /// True if someone is currently transmitting
    tx_active: bool,
    /// When PTT was released (for hangtime). None if transmitting.
    hangtime_start: Option<TdmaTime>,
    /// Brew session UUID — set when a network speaker is active on this call,
    /// regardless of call origin. Cleared when the network speaker ends.
    brew_uuid: Option<uuid::Uuid>,
}

impl CcBsSubentity {
    pub fn new(config: SharedConfig) -> Self {
        CcBsSubentity {
            config,
            dltime: TdmaTime::default(),
            cached_setups: HashMap::new(),
            circuits: CircuitMgr::new(),
            active_calls: HashMap::new(),
            subscriber_groups: HashMap::new(),
            group_listeners: HashMap::new(),
            releasing_calls: Vec::new(),
            individual_calls: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
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

    fn has_listener(&self, gssi: u32) -> bool {
        self.group_listeners.get(&gssi).copied().unwrap_or(0) > 0
    }

    fn inc_group_listener(&mut self, gssi: u32) {
        let entry = self.group_listeners.entry(gssi).or_insert(0);
        *entry += 1;
    }

    fn dec_group_listener(&mut self, gssi: u32) {
        if let Some(entry) = self.group_listeners.get_mut(&gssi) {
            if *entry <= 1 {
                self.group_listeners.remove(&gssi);
            } else {
                *entry -= 1;
            }
        }
    }

    fn drop_group_calls_if_unlistened(&mut self, queue: &mut MessageQueue, gssi: u32) {
        if self.has_listener(gssi) {
            return;
        }

        let to_drop: Vec<(u16, CallOrigin)> = self
            .active_calls
            .iter()
            .filter(|(_, call)| call.dest_gssi == gssi)
            .map(|(call_id, call)| (*call_id, call.origin.clone()))
            .collect();

        for (call_id, origin) in to_drop {
            tracing::info!("CMCE: dropping call_id={} gssi={} (no listeners)", call_id, gssi);
            if let CallOrigin::Network { brew_uuid } = origin {
                if net_brew::is_brew_gssi_routable(&self.config, gssi) {
                    queue.push_back(SapMsg {
                        sap: Sap::Control,
                        src: TetraEntity::Cmce,
                        dest: TetraEntity::Brew,
                        msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid }),
                    });
                };
            };
            self.release_call(queue, call_id, DisconnectCause::SwmiRequestedDisconnection);
        }
    }

    pub fn handle_subscriber_update(&mut self, queue: &mut MessageQueue, update: MmSubscriberUpdate) {
        let issi = update.issi;
        let groups = update.groups;

        match update.action {
            BrewSubscriberAction::Register => {
                let known = self.subscriber_groups.contains_key(&issi);
                self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                tracing::info!("CMCE: subscriber register issi={} known={}", issi, known);
            }
            BrewSubscriberAction::Deregister => {
                if let Some(existing) = self.subscriber_groups.remove(&issi) {
                    for gssi in existing {
                        self.dec_group_listener(gssi);
                        self.drop_group_calls_if_unlistened(queue, gssi);
                    }
                }
                tracing::info!("CMCE: subscriber deregister issi={}", issi);
            }
            BrewSubscriberAction::Affiliate => {
                let mut new_groups = Vec::new();
                {
                    let entry = self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                    for gssi in groups {
                        if entry.insert(gssi) {
                            new_groups.push(gssi);
                        }
                    }
                }
                for gssi in &new_groups {
                    self.inc_group_listener(*gssi);
                }

                if new_groups.is_empty() {
                    tracing::debug!("CMCE: affiliate ignored (no new groups) issi={}", issi);
                } else {
                    tracing::info!("CMCE: subscriber affiliate issi={} groups={:?}", issi, new_groups);
                }
            }
            BrewSubscriberAction::Deaffiliate => {
                let mut removed_groups = Vec::new();
                let mut known_issi = false;
                if let Some(entry) = self.subscriber_groups.get_mut(&issi) {
                    known_issi = true;
                    for gssi in groups {
                        if entry.remove(&gssi) {
                            removed_groups.push(gssi);
                        }
                    }
                } else {
                    removed_groups = groups;
                }
                if known_issi {
                    for gssi in &removed_groups {
                        self.dec_group_listener(*gssi);
                    }
                }

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

    fn signal_umac_circuit_open(queue: &mut MessageQueue, call: &CmceCircuit, peer_ts: Option<u8>, dl_media_source: CircuitDlMediaSource) {
        let circuit = Circuit {
            direction: call.direction,
            ts: call.ts,
            peer_ts,
            usage: call.usage,
            circuit_mode: call.circuit_mode,
            speech_service: call.speech_service,
            etee_encrypted: call.etee_encrypted,
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

    fn signal_umac_circuit_close(queue: &mut MessageQueue, circuit: CmceCircuit) {
        let cmd = SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::Close(circuit.direction, circuit.ts)),
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

        // Allocate circuit (DL+UL for group call)
        let circuit = match {
            let mut state = self.config.state_write();
            self.circuits.allocate_circuit_with_allocator(
                Direction::Both,
                pdu.basic_service_information.communication_type,
                &mut state.timeslot_alloc,
                TimeslotOwner::Cmce,
            )
        } {
            Ok(circuit) => circuit.clone(),
            Err(e) => {
                tracing::error!("Failed to allocate circuit for U-SETUP: {:?}", e);
                return;
            }
        };

        tracing::info!(
            "rx_u_setup: call from ISSI {} to GSSI {} → ts={} call_id={} usage={}",
            calling_party.ssi,
            dest_gssi,
            circuit.ts,
            circuit.call_id,
            circuit.usage
        );

        // Signal UMAC to open DL+UL circuits
        Self::signal_umac_circuit_open(queue, &circuit, None, CircuitDlMediaSource::LocalLoopback);

        // Build channel allocation timeslot mask for this call
        let mut timeslots = [false; 4];
        timeslots[circuit.ts as usize - 1] = true;

        // Extract UL message routing info (handle, link_id, endpoint_id) for
        // individually-addressed responses. These are needed so MLE can route
        // the response back to the correct radio via the established LLC link.
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {
            panic!()
        };
        let ul_handle = prim.handle;
        let ul_link_id = prim.link_id;
        let ul_endpoint_id = prim.endpoint_id;

        // === 1) Send D-CALL-PROCEEDING to the calling MS (individually addressed) ===
        // This acknowledges the U-SETUP and keeps the radio from timing out.
        self.send_d_call_proceeding(queue, &message, &pdu, circuit.call_id);

        // === 2) Send D-CONNECT to the calling MS with Granted + channel allocation ===
        // This transitions the calling MS from "Call Setup" to "Active".
        // MUST be sent BEFORE the group D-SETUP so the radio receives it on MCCH.
        // Uses the correct MLE handle (not 0) so MLE routes it properly.
        let d_connect = DConnect {
            call_identifier: circuit.call_id,
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
                    usage: Some(circuit.usage),
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
            call_identifier: circuit.call_id,
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

        // Cache for late-entry re-sends. Receipt starts as None so the CircuitMgr-triggered
        // backup send (within D_SETUP_REPEATS frames) is not throttled by this initial send.
        // The first re-send via tick_start will create a tracked receipt.
        self.cached_setups.insert(circuit.call_id, (d_setup, dest_addr, None));
        let (d_setup_ref, _, _) = self.cached_setups.get(&circuit.call_id).unwrap();

        let (setup_sdu, setup_chan_alloc) = Self::build_d_setup_prim(d_setup_ref, circuit.usage, circuit.ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, Some(setup_chan_alloc), dest_addr, Layer2Service::Unacknowledged, None);
        queue.push_back(setup_msg);

        // Track the active local call — caller is granted the floor, so tx_active = true
        self.active_calls.insert(
            circuit.call_id,
            ActiveCall {
                origin: CallOrigin::Local {
                    caller_addr: calling_party,
                },
                dest_gssi,
                source_issi: calling_party.ssi,
                ts: circuit.ts,
                usage: circuit.usage,
                tx_active: true,
                hangtime_start: None,
                brew_uuid: None,
            },
        );

        // Notify Brew entity about this local call if Brew is loaded and the SSI is cleared for Brew
        // It can then forward to TetraPack if the group is subscribed
        if net_brew::is_brew_gssi_routable(&self.config, dest_gssi) {
            let msg = SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id: circuit.call_id,
                    source_issi: calling_party.ssi,
                    dest_gssi,
                    ts: circuit.ts,
                }),
            };
            queue.push_back(msg);
        }
    }

    /// True if the ISSI is registered on this cell, so we can reach it for a local call.
    fn is_individual_registered(&self, issi: u32) -> bool {
        self.subscriber_groups.contains_key(&issi)
    }

    /// True if the ISSI is already a party to an individual call.
    fn issi_in_individual_call(&self, issi: u32) -> bool {
        self.individual_calls
            .values()
            .any(|c| c.calling_addr.ssi == issi || c.called_addr.ssi == issi)
    }

    /// Send a downlink PDU to the calling party over its established LLC link.
    fn send_to_caller(&self, queue: &mut MessageQueue, call: &IndividualCall, sdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>) {
        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: call.calling_handle,
                endpoint_id: call.calling_endpoint_id,
                link_id: call.calling_link_id,
                layer2service: Layer2Service::Unacknowledged,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc,
                main_address: call.calling_addr,
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
        let calling_circuit = match {
            let mut state = self.config.state_write();
            self.circuits
                .allocate_circuit_with_allocator(Direction::Both, comm_type, &mut state.timeslot_alloc, TimeslotOwner::Cmce)
        } {
            Ok(circuit) => circuit.clone(),
            Err(e) => {
                tracing::error!("Failed to allocate circuit for individual U-SETUP: {:?}", e);
                self.reject_individual_setup(queue, message, DisconnectCause::CongestionInInfrastructure);
                return;
            }
        };
        // A duplex call needs a second channel so the called party can transmit at the same
        // time as the caller. Simplex shares one channel (both parties on the same slot).
        let called_circuit = if duplex {
            match {
                let mut state = self.config.state_write();
                self.circuits
                    .allocate_circuit_with_allocator(Direction::Both, comm_type, &mut state.timeslot_alloc, TimeslotOwner::Cmce)
            } {
                Ok(circuit) => circuit.clone(),
                Err(e) => {
                    tracing::error!("Failed to allocate second circuit for duplex U-SETUP: {:?}", e);
                    let _ = self.circuits.close_circuit(Direction::Both, calling_circuit.ts);
                    self.release_timeslot(calling_circuit.ts);
                    self.reject_individual_setup(queue, message, DisconnectCause::CongestionInInfrastructure);
                    return;
                }
            }
        } else {
            calling_circuit.clone()
        };

        let calling_addr = calling_party;
        let called_addr = TetraAddress::new(called_ssi, SsiType::Issi);
        let hook_on_off = pdu.hook_method_selection;

        tracing::info!(
            "individual call ISSI {} to ISSI {} ts={} called_ts={} call_id={} hook_on_off={} duplex={}",
            calling_ssi,
            called_ssi,
            calling_circuit.ts,
            called_circuit.ts,
            calling_circuit.call_id,
            hook_on_off,
            duplex
        );

        // Open the traffic channel(s). For duplex, cross-link the two slots so each party's
        // uplink voice loops to the other party's downlink.
        if duplex {
            Self::signal_umac_circuit_open(
                queue,
                &calling_circuit,
                Some(called_circuit.ts),
                CircuitDlMediaSource::LocalLoopback,
            );
            Self::signal_umac_circuit_open(
                queue,
                &called_circuit,
                Some(calling_circuit.ts),
                CircuitDlMediaSource::LocalLoopback,
            );
        } else {
            Self::signal_umac_circuit_open(queue, &calling_circuit, None, CircuitDlMediaSource::LocalLoopback);
        }

        // D-CALL-PROCEEDING acknowledges the U-SETUP to the caller.
        self.send_d_call_proceeding(queue, message, &pdu, calling_circuit.call_id);

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

        // D-SETUP to the called party. No channel allocation here: in a hangtime
        // (quasi-transmission-trunked) call ETSI Table 14.1 does not allow early
        // assignment, so the called MS stays on the control channel and answers there.
        // The traffic channel is assigned later in the D-CONNECT ACKNOWLEDGE.
        let d_setup = DSetup {
            call_identifier: calling_circuit.call_id,
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
        let (setup_sdu, _) = Self::build_d_setup_prim(&d_setup, called_circuit.usage, called_circuit.ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, None, called_addr, Layer2Service::Unacknowledged, None);
        queue.push_back(setup_msg);

        self.individual_calls.insert(
            calling_circuit.call_id,
            IndividualCall {
                call_id: calling_circuit.call_id,
                calling_addr,
                called_addr,
                calling_handle: handle,
                calling_link_id: link_id,
                calling_endpoint_id: endpoint_id,
                ts: calling_circuit.ts,
                usage: calling_circuit.usage,
                called_ts: called_circuit.ts,
                called_usage: called_circuit.usage,
                duplex,
                over_brew: false,
                brew_uuid: None,
                hook_on_off,
                state: IndividualCallState::SetupSent,
                floor_holder,
                phase_started: self.dltime,
            },
        );
    }

    /// Find the call id of an over-Brew individual call by its Brew session UUID.
    fn individual_by_brew_uuid(&self, brew_uuid: uuid::Uuid) -> Option<u16> {
        self.individual_calls
            .iter()
            .find(|(_, c)| c.brew_uuid == Some(brew_uuid))
            .map(|(id, _)| *id)
    }

    /// Decode an external subscriber number Type3 element into a dial string. The digits are
    /// packed as 4-bit BCD nibbles, most significant first, in the element's data word.
    fn decode_external_subscriber_number(field: &tetra_core::typed_pdu_fields::Type3FieldGeneric) -> String {
        let nibble_count = field.len / 4;
        let mut digits = String::with_capacity(nibble_count);
        for i in 0..nibble_count {
            let shift = field.len - 4 * (i + 1);
            let nibble = ((field.data >> shift) & 0xf) as u8;
            match nibble {
                0..=9 => digits.push(char::from(b'0' + nibble)),
                0x0a => digits.push('*'),
                0x0b => digits.push('#'),
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

        let circuit = match {
            let mut state = self.config.state_write();
            self.circuits.allocate_circuit_with_allocator(
                Direction::Both,
                pdu.basic_service_information.communication_type,
                &mut state.timeslot_alloc,
                TimeslotOwner::Cmce,
            )
        } {
            Ok(circuit) => circuit.clone(),
            Err(e) => {
                tracing::error!("Failed to allocate circuit for over-Brew U-SETUP: {:?}", e);
                self.reject_individual_setup(queue, message, DisconnectCause::CongestionInInfrastructure);
                return;
            }
        };

        let brew_uuid = uuid::Uuid::new_v4();
        tracing::info!(
            "individual call over Brew: ISSI {} to dest={} number='{}' ts={} call_id={} duplex={} uuid={}",
            calling_ssi,
            called_ssi,
            number,
            circuit.ts,
            circuit.call_id,
            duplex,
            brew_uuid
        );

        // Open the traffic channel now with network downlink media so the local loopback is
        // suppressed. Audio comes from the backend, the caller's uplink goes to the backend.
        Self::signal_umac_circuit_open(queue, &circuit, None, CircuitDlMediaSource::Network);

        self.send_d_call_proceeding(queue, message, pdu, circuit.call_id);

        let call = NetworkCircuitCall {
            source_issi: calling_ssi,
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
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Brew,
            msg: SapMsgInner::CmceCallControl(CallControl::NetworkCircuitSetupRequest { brew_uuid, call }),
        });

        self.individual_calls.insert(
            circuit.call_id,
            IndividualCall {
                call_id: circuit.call_id,
                calling_addr: calling_party,
                called_addr: TetraAddress::new(called_ssi, SsiType::Issi),
                calling_handle: handle,
                calling_link_id: link_id,
                calling_endpoint_id: endpoint_id,
                ts: circuit.ts,
                usage: circuit.usage,
                called_ts: circuit.ts,
                called_usage: circuit.usage,
                duplex,
                over_brew: true,
                brew_uuid: Some(brew_uuid),
                hook_on_off: pdu.hook_method_selection,
                state: IndividualCallState::SetupSent,
                // Duplex grants both. Simplex over Brew: the backend drives the floor with
                // SIMPLEX GRANTED/IDLE, so start with nobody holding it.
                floor_holder: None,
                phase_started: self.dltime,
            },
        );
    }

    /// Backend is alerting (ringing) on an over-Brew call. Relay D-ALERT to the caller.
    fn rx_network_circuit_alert(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid) {
        let Some(call_id) = self.individual_by_brew_uuid(brew_uuid) else {
            return;
        };
        let call = self.individual_calls.get_mut(&call_id).unwrap();
        if call.state == IndividualCallState::SetupSent {
            call.state = IndividualCallState::Alerting;
            call.phase_started = self.dltime;
        }
        let call = call.clone();
        let d_alert = DAlert {
            call_identifier: call.call_id,
            call_time_out_set_up_phase: 0,
            reserved: false,
            simplex_duplex_selection: call.duplex,
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
    fn brew_simplex_floor(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid, caller_talks: bool) {
        let Some(call_id) = self.individual_by_brew_uuid(brew_uuid) else {
            return;
        };
        let Some(call) = self.individual_calls.get(&call_id) else {
            return;
        };
        if call.duplex {
            return; // Duplex has no floor cycle.
        }
        let caller = call.calling_addr;
        let ts = call.ts;
        let grant = if caller_talks {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };
        self.individual_calls.get_mut(&call_id).unwrap().floor_holder = if caller_talks { Some(caller.ssi) } else { None };
        self.send_individual_tx_granted(queue, call_id, caller.ssi, caller, grant, ts);
    }

    /// Backend connected an over-Brew call. Through-connect the local caller: D-CONNECT with
    /// the channel allocation, then tell Brew media is ready and confirm the connect.
    fn rx_network_circuit_connect_request(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid) {
        let Some(call_id) = self.individual_by_brew_uuid(brew_uuid) else {
            return;
        };
        let call = self.individual_calls.get_mut(&call_id).unwrap();
        if call.state == IndividualCallState::Active {
            return;
        }
        call.state = IndividualCallState::Active;
        call.phase_started = self.dltime;
        let call = call.clone();

        let mut timeslots = [false; 4];
        timeslots[call.ts as usize - 1] = true;
        let chan_alloc = CmceChanAllocReq {
            usage: Some(call.usage),
            alloc_type: ChanAllocType::Replace,
            carrier: None,
            timeslots,
            ul_dl_assigned: UlDlAssignment::Both,
        };
        let d_connect = DConnect {
            call_identifier: call.call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: call.hook_on_off,
            simplex_duplex_selection: call.duplex,
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

        // Tell Brew the local media slot, then confirm the connect.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Brew,
            msg: SapMsgInner::CmceCallControl(CallControl::NetworkCircuitMediaReady {
                brew_uuid,
                call_id: call.call_id,
                ts: call.ts,
            }),
        });
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Brew,
            msg: SapMsgInner::CmceCallControl(CallControl::NetworkCircuitConnectConfirm {
                brew_uuid,
                grant: TransmissionGrant::Granted.into_raw() as u8,
                permission: 0,
            }),
        });
        tracing::info!("individual call over Brew call_id={} active", call.call_id);
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
        let Some(call) = self.individual_calls.get_mut(&pdu.call_identifier) else {
            tracing::warn!("U-ALERT for unknown individual call_id={}", pdu.call_identifier);
            return;
        };
        if call.state != IndividualCallState::SetupSent || !call.hook_on_off {
            tracing::warn!("U-ALERT ignored for call_id={} in state {:?}", call.call_id, call.state);
            return;
        }
        call.state = IndividualCallState::Alerting;
        call.phase_started = self.dltime;
        let call = call.clone();

        // D-ALERT to caller. The old hook field is now Reserved and shall be 1 (ETSI Table 14.4).
        let d_alert = DAlert {
            call_identifier: call.call_id,
            call_time_out_set_up_phase: CallTimeoutSetupPhase::T60s as u8,
            reserved: true,
            simplex_duplex_selection: false,
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
        let Some(call) = self.individual_calls.get_mut(&pdu.call_identifier) else {
            tracing::warn!("U-CONNECT for unknown individual call_id={}", pdu.call_identifier);
            return;
        };
        if call.state == IndividualCallState::Active {
            tracing::warn!("U-CONNECT for already-active call_id={}, ignoring", pdu.call_identifier);
            return;
        }
        call.state = IndividualCallState::Active;
        call.phase_started = self.dltime;
        let call = call.clone();

        let caller_has_floor = call.floor_holder == Some(call.calling_addr.ssi);

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
        let caller_grant = if call.duplex || caller_has_floor {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };
        let called_grant = if call.duplex || !caller_has_floor {
            TransmissionGrant::Granted
        } else {
            TransmissionGrant::GrantedToOtherUser
        };

        // D-CONNECT to the caller with its channel allocation. The caller owns the call.
        let d_connect = DConnect {
            call_identifier: call.call_id,
            call_time_out: CallTimeout::T5m,
            hook_method_selection: call.hook_on_off,
            simplex_duplex_selection: call.duplex,
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
        self.send_to_caller(queue, &call, connect_sdu, Some(make_chan_alloc(call.ts, call.usage)));

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
            Some(make_chan_alloc(call.called_ts, call.called_usage)),
            call.called_addr,
            Layer2Service::Unacknowledged,
            None,
        ));

        // Put the timeslot in traffic mode for the initial floor holder so its uplink
        // voice is looped to the peer on the downlink.
        if let Some(holder) = call.floor_holder {
            let peer = if holder == call.calling_addr.ssi {
                call.called_addr.ssi
            } else {
                call.calling_addr.ssi
            };
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id: call.call_id,
                    source_issi: holder,
                    dest_gssi: peer,
                    ts: call.ts,
                }),
            });
        }

        tracing::info!("individual call_id={} active", call.call_id);
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
        let Some(call) = self.individual_calls.remove(&call_id) else {
            return;
        };
        // Once active both parties are on the traffic channel, so steal the D-RELEASE onto
        // it. During setup or alerting they are still on the control channel, so send it
        // there, otherwise a reject or caller cancel never reaches the other party.
        let on_traffic = call.state == IndividualCallState::Active;
        // Each local party is on its own slot (the same slot for simplex). An over-Brew call
        // has only the local caller; the called leg is the backend and gets no on-air release.
        let mut legs = vec![(call.calling_addr, call.ts)];
        if !call.over_brew {
            legs.push((call.called_addr, call.called_ts));
        }
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

        if call.over_brew && notify_brew {
            if let Some(brew_uuid) = call.brew_uuid {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    msg: SapMsgInner::CmceCallControl(CallControl::NetworkCircuitRelease {
                        brew_uuid,
                        cause: cause as u8,
                    }),
                });
            }
        }

        // Defer teardown so the stolen D-RELEASE goes out. dest_gssi=0 keeps the Brew
        // notifications in finalize_release inert for a local individual call. A local duplex
        // call also frees its second slot (over-Brew uses one slot).
        self.releasing_calls.push(ReleasingCall {
            call_id,
            ts: call.ts,
            peer_ts: if call.duplex && !call.over_brew {
                Some(call.called_ts)
            } else {
                None
            },
            dest_gssi: 0,
            is_local: true,
            brew_uuid: None,
            sent_at: self.dltime,
        });
    }

    /// Release individual calls that pass their setup/no-answer or call-length timeout.
    fn process_individual_timeouts(&mut self, queue: &mut MessageQueue) {
        // ETSI 14.6: T303 calling set-up timer 60 s, T310 call length min 30 s.
        // Values in timeslots, since TdmaTime ages in timeslots (~14 ms each).
        const SETUP_TIMEOUT_TS: i32 = 4235; // ~60 s
        const ACTIVE_TIMEOUT_TS: i32 = 21176; // ~300 s

        let now = self.dltime;
        let expired: Vec<u16> = self
            .individual_calls
            .iter()
            .filter_map(|(id, c)| {
                let limit = if c.state == IndividualCallState::Active {
                    ACTIVE_TIMEOUT_TS
                } else {
                    SETUP_TIMEOUT_TS
                };
                (c.phase_started.age(now) >= limit).then_some(*id)
            })
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
        let Some(call) = self.individual_calls.get_mut(&call_id) else {
            return;
        };
        let ts = call.ts;
        let addrs: Vec<TetraAddress> = if call.over_brew {
            vec![call.calling_addr]
        } else {
            vec![call.calling_addr, call.called_addr]
        };
        call.floor_holder = None;

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
        let Some(call) = self.individual_calls.get(&call_id) else {
            return;
        };
        let (calling, called) = (call.calling_addr, call.called_addr);
        if requester != calling.ssi && requester != called.ssi {
            tracing::warn!("U-TX DEMAND from non-party ISSI {} on call_id={}", requester, call_id);
            return;
        }
        // Wait for the current talker to cease before granting (ETSI 14.5.1.2.1 a).
        if let Some(holder) = call.floor_holder {
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
        let ts = call.ts;
        let over_brew = call.over_brew;
        let (requester_addr, peer) = if requester == calling.ssi {
            (calling, called)
        } else {
            (called, calling)
        };
        self.individual_calls.get_mut(&call_id).unwrap().floor_holder = Some(requester);

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
                        // Individual calls are point-to-point, so there is no late entry and
                        // no cached D-SETUP to resend. Match on the timeslot so a duplex call's
                        // second slot is covered too.
                        if self.individual_calls.values().any(|c| c.ts == ts || c.called_ts == ts) {
                            continue;
                        }
                        // Skip late-entry D-SETUP during hangtime. The traffic channel is still
                        // allocated and sending D-SETUP with NotGranted can prevent floor requests.
                        if let Some(active) = self.active_calls.get(&call_id) {
                            if active.hangtime_start.is_some() {
                                continue;
                            }
                        }

                        // Get our cached D-SETUP, build a prim and send it down the stack
                        let Some((pdu, dest_addr, receipt)) = self.cached_setups.get_mut(&call_id) else {
                            tracing::error!("No cached D-SETUP for call id {}", call_id);
                            continue;
                        };

                        // Throttle: if the previous D-SETUP hasn't reached a final state yet
                        // (still queued in UMAC), skip this re-send to avoid flooding the MCCH.
                        if let Some(r) = receipt.as_ref() {
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
                        if let Some(active) = self.active_calls.get(&call_id) {
                            pdu.transmission_grant = if active.tx_active {
                                TransmissionGrant::GrantedToOtherUser
                            } else {
                                TransmissionGrant::NotGranted
                            };
                        }
                        let dest_addr = *dest_addr;
                        let (sdu, chan_alloc) = Self::build_d_setup_prim(pdu, usage, ts, UlDlAssignment::Both);

                        // Create a fresh txreporter for this re-send
                        let reporter = TxReporter::new_unacked();

                        // Cache the setup in cached_setups with the reporter so we can check its state on the next tick and throttle if it's still pending in UMAC
                        *receipt = Some(reporter.clone());

                        let prim = Self::build_sapmsg(sdu, Some(chan_alloc), dest_addr, Layer2Service::Unacknowledged, Some(reporter));
                        queue.push_back(prim);
                    }

                    CircuitMgrCmd::SendClose(call_id, circuit) => {
                        tracing::warn!("need to send CLOSE for call id {}", call_id);
                        let ts = circuit.ts;
                        // Get our cached D-SETUP, build D-RELEASE and send
                        if let Some((pdu, dest_addr, _)) = self.cached_setups.get(&call_id) {
                            let dest_addr = *dest_addr;
                            let sdu = Self::build_d_release_from_d_setup(pdu, DisconnectCause::ExpiryOfTimer);
                            let prim = Self::build_sapmsg(sdu, None, dest_addr, Layer2Service::Unacknowledged, None);
                            queue.push_back(prim);
                        } else {
                            tracing::error!("No cached D-SETUP for call id {}", call_id);
                        }

                        // Clean up call state
                        self.cached_setups.remove(&call_id);
                        self.active_calls.remove(&call_id);

                        // Signal UMAC to release the circuit
                        Self::signal_umac_circuit_close(queue, circuit);
                        self.release_timeslot(ts);
                    }
                }
            }
        }
    }

    /// Check if any active calls in hangtime have expired, and if so, release them
    fn check_hangtime_expiry(&mut self, queue: &mut MessageQueue) {
        // Hangtime: 5 multiframes = ~5 seconds
        const HANGTIME_FRAMES: i32 = 5 * 18 * 4;

        let expired: Vec<u16> = self
            .active_calls
            .iter()
            .filter_map(|(&call_id, call)| {
                if let Some(hangtime_start) = call.hangtime_start {
                    if hangtime_start.age(self.dltime) > HANGTIME_FRAMES {
                        return Some(call_id);
                    }
                }
                None
            })
            .collect();

        for call_id in expired {
            tracing::info!("Hangtime expired for call_id={}, releasing", call_id);
            self.release_call(queue, call_id, DisconnectCause::ExpiryOfTimer);
        }
    }

    fn release_timeslot(&mut self, ts: u8) {
        let mut state = self.config.state_write();
        if let Err(err) = state.timeslot_alloc.release(TimeslotOwner::Cmce, ts) {
            tracing::warn!("CcBsSubentity: failed to release timeslot ts={} err={:?}", ts, err);
        }
    }

    /// Release a call. Removes it from active state immediately so it cannot be re-keyed
    /// or reused, steals one D-RELEASE onto the traffic channel, and parks the teardown in
    /// releasing_calls. The circuit and timeslot stay allocated until process_releasing_calls
    /// tears them down, which lets the stolen D-RELEASE transmit before the slot leaves
    /// traffic mode. With no cached D-SETUP there is no D-RELEASE to send, so it tears down
    /// at once.
    fn release_call(&mut self, queue: &mut MessageQueue, call_id: u16, disconnect_cause: DisconnectCause) {
        let Some(call) = self.active_calls.remove(&call_id) else {
            return;
        };
        let ts = call.ts;
        let dest_gssi = call.dest_gssi;
        let is_local = matches!(call.origin, CallOrigin::Local { .. });
        // Prefer the live brew_uuid (current network speaker). Fall back to the origin uuid
        // for a Network call in hangtime, where rx_network_call_end cleared the field.
        let brew_uuid = call.brew_uuid.or(match call.origin {
            CallOrigin::Network { brew_uuid } => Some(brew_uuid),
            CallOrigin::Local { .. } => None,
        });

        match self.cached_setups.remove(&call_id) {
            Some((d_setup, dest_addr, _)) => {
                let sdu = Self::build_d_release_from_d_setup(&d_setup, disconnect_cause);
                queue.push_back(Self::build_sapmsg_stealing(sdu, dest_addr, ts));
                self.releasing_calls.push(ReleasingCall {
                    call_id,
                    ts,
                    peer_ts: None,
                    dest_gssi,
                    is_local,
                    brew_uuid,
                    sent_at: self.dltime,
                });
            }
            None => {
                tracing::warn!("No cached D-SETUP for call_id={}, cleaning up without D-RELEASE", call_id);
                self.finalize_release(queue, call_id, ts, dest_gssi, is_local, brew_uuid);
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
        let mut i = 0;
        while i < self.releasing_calls.len() {
            if self.releasing_calls[i].sent_at.age(now) >= CLOSE_AFTER_SEND_TS {
                let rc = self.releasing_calls.remove(i);
                if let Some(peer_ts) = rc.peer_ts {
                    // Free the duplex call's second slot too.
                    if let Ok(circuit) = self.circuits.close_circuit(Direction::Both, peer_ts) {
                        Self::signal_umac_circuit_close(queue, circuit);
                    }
                    queue.push_back(SapMsg {
                        sap: Sap::Control,
                        src: TetraEntity::Cmce,
                        dest: TetraEntity::Umac,
                        msg: SapMsgInner::CmceCallControl(CallControl::CallEnded {
                            call_id: rc.call_id,
                            ts: peer_ts,
                        }),
                    });
                    self.release_timeslot(peer_ts);
                }
                self.finalize_release(queue, rc.call_id, rc.ts, rc.dest_gssi, rc.is_local, rc.brew_uuid);
            } else {
                i += 1;
            }
        }
    }

    /// Tear down a released call: close the circuit, free the timeslot, notify Brew.
    /// The active_calls and cached_setups entries were already removed in release_call.
    fn finalize_release(
        &mut self,
        queue: &mut MessageQueue,
        call_id: u16,
        ts: u8,
        dest_gssi: u32,
        is_local: bool,
        brew_uuid: Option<uuid::Uuid>,
    ) {
        if let Ok(circuit) = self.circuits.close_circuit(Direction::Both, ts) {
            Self::signal_umac_circuit_close(queue, circuit);
        }

        // Ensure UMAC clears hangtime even if the CMCE circuit was already closed above.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            msg: SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts }),
        });

        self.release_timeslot(ts);

        // Tell Brew the call is gone. Local-origin calls get CallEnded so Brew clears
        // ul_forwarded[ts] from any earlier UL forwarding. If a network speaker was
        // ever involved (active or in hangtime), also send NetworkCallEnd so Brew
        // tears down the upstream session. Without the latter, hangtime expiry leaves
        // Brew thinking the circuit is still reusable and the backend keeps streaming.
        if net_brew::is_brew_gssi_routable(&self.config, dest_gssi) {
            if is_local {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    msg: SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts }),
                });
            }
            if let Some(brew_uuid) = brew_uuid {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid }),
                });
            }
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
        if let Some(call) = self.individual_calls.get(&call_id) {
            if !call.duplex {
                self.individual_tx_ceased(queue, call_id);
            }
            return;
        }

        // Look up the active call
        let Some(call) = self.active_calls.get_mut(&call_id) else {
            tracing::warn!("U-TX CEASED for unknown call_id={}", call_id);
            return;
        };

        // Check if already in hangtime - ignore duplicate U-TX CEASED to avoid resetting timer
        if !call.tx_active && call.hangtime_start.is_some() {
            tracing::debug!("U-TX CEASED: already in hangtime for call_id={}, ignoring duplicate", call_id);
            return;
        }

        tracing::info!("U-TX CEASED: PTT released on call_id={}, entering hangtime", call_id);

        let ts = call.ts;
        let dest_ssi = call.dest_gssi;
        call.tx_active = false;
        call.hangtime_start = Some(self.dltime);

        // Get dest address from cached setup
        let Some((_, dest_addr, _)) = self.cached_setups.get(&call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = *dest_addr;

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

        // Notify Brew to stop forwarding audio, if this SSI is cleared for Br
        if net_brew::is_brew_gssi_routable(&self.config, dest_ssi) {
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
            });
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
        if let Some(call) = self.individual_calls.get(&call_id) {
            if !call.duplex {
                self.individual_tx_demand(queue, call_id, requesting_party.ssi);
            }
            return;
        }

        let Some(call) = self.active_calls.get_mut(&call_id) else {
            tracing::warn!("U-TX DEMAND for unknown call_id={}", call_id);
            return;
        };

        tracing::info!("U-TX DEMAND: ISSI {} requests floor on call_id={}", requesting_party.ssi, call_id);

        // ETSI 14.5.2.2.1 b): if another MS is already transmitting, the SwMI should
        // normally wait for that party to finish before granting. Reject the request.
        if call.tx_active {
            tracing::warn!(
                "U-TX DEMAND from ISSI {} rejected, ISSI {} already transmitting on call_id={}",
                requesting_party.ssi,
                call.source_issi,
                call_id
            );
            return;
        }

        // Grant the floor to the requesting MS
        let ts = call.ts;
        call.tx_active = true;
        call.hangtime_start = None;
        call.source_issi = requesting_party.ssi;

        // Update caller_addr for local calls
        if let CallOrigin::Local { caller_addr } = &mut call.origin {
            *caller_addr = requesting_party;
        }

        let Some((_, dest_addr, _)) = self.cached_setups.get(&call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = *dest_addr;

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
            let Some(call) = self.active_calls.get(&call_id) else {
                return;
            };
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id,
                    source_issi: requesting_party.ssi,
                    dest_gssi: dest_addr.ssi,
                    ts: call.ts,
                }),
            });
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
        if self.individual_calls.contains_key(&call_id) {
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
        if self.individual_calls.contains_key(&call_id) {
            tracing::info!("U-DISCONNECT: ISSI {} disconnecting individual call_id={}", sender.ssi, call_id);
            self.release_individual_call(queue, call_id, DisconnectCause::UserRequestedDisconnection);
            return;
        }

        let Some(call) = self.active_calls.get(&call_id) else {
            tracing::debug!("U-DISCONNECT for unknown call_id={} (likely duplicate)", call_id);
            return;
        };

        let is_call_owner = matches!(&call.origin, CallOrigin::Local { caller_addr } if caller_addr.ssi == sender.ssi);

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

    /// Handle incoming CallControl messages from Brew
    pub fn rx_call_control(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        let SapMsgInner::CmceCallControl(call_control) = message.msg else {
            panic!("Expected CmceCallControl message");
        };

        match call_control {
            CallControl::NetworkCallStart {
                brew_uuid,
                source_issi,
                dest_gssi,
                priority,
            } => {
                self.rx_network_call_start(queue, brew_uuid, source_issi, dest_gssi, priority);
            }
            CallControl::NetworkCallEnd { brew_uuid } => {
                self.rx_network_call_end(queue, brew_uuid);
            }
            CallControl::UlInactivityTimeout { ts } => {
                self.handle_ul_inactivity_timeout(queue, ts);
            }
            CallControl::NetworkCircuitSetupAccept { brew_uuid } => {
                tracing::debug!("over-Brew call setup accepted uuid={}", brew_uuid);
            }
            CallControl::NetworkCircuitAlert { brew_uuid } => {
                self.rx_network_circuit_alert(queue, brew_uuid);
            }
            CallControl::NetworkCircuitConnectRequest { brew_uuid, .. } => {
                self.rx_network_circuit_connect_request(queue, brew_uuid);
            }
            CallControl::NetworkCircuitSetupReject { brew_uuid, cause } | CallControl::NetworkCircuitRelease { brew_uuid, cause } => {
                if let Some(call_id) = self.individual_by_brew_uuid(brew_uuid) {
                    let disconnect_cause = DisconnectCause::try_from(cause as u64).unwrap_or(DisconnectCause::CallRejectedByTheCalledParty);
                    // The teardown came from Brew, so do not echo a release back to it.
                    self.release_individual_call_inner(queue, call_id, disconnect_cause, false);
                }
            }
            CallControl::NetworkCircuitSimplexGranted { brew_uuid, .. } => {
                // Far party (backend) holds the floor: the local caller switches to receive.
                self.brew_simplex_floor(queue, brew_uuid, false);
            }
            CallControl::NetworkCircuitSimplexIdle { brew_uuid, .. } => {
                // Floor free: grant it to the local caller so it can talk.
                self.brew_simplex_floor(queue, brew_uuid, true);
            }
            _ => {
                tracing::warn!("Unexpected CallControl message: {:?}", call_control);
            }
        }
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

            // We already checked this is cleared for brew
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid }),
            });
            return;
        }

        // Check if there is an active call for this GSSI (speaker change scenario)
        if let Some((call_id, call)) = self.active_calls.iter_mut().find(|(_, c)| c.dest_gssi == dest_gssi) {
            // Reject speaker change if a local MS is already transmitting
            if call.tx_active {
                tracing::warn!(
                    "CMCE: network speaker change rejected, ISSI {} already transmitting on gssi={}",
                    call.source_issi,
                    dest_gssi
                );
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid }),
                });
                return;
            }

            // Speaker change during hangtime
            tracing::info!(
                "CMCE: network call speaker change gssi={} new_speaker={} (was {})",
                dest_gssi,
                source_issi,
                call.source_issi
            );

            call.source_issi = source_issi;
            call.tx_active = true;
            call.hangtime_start = None;
            call.brew_uuid = Some(brew_uuid);

            if let CallOrigin::Network { brew_uuid: old_uuid } = call.origin {
                // Backend issues a fresh UUID for each speaker, so this fires every change.
                if old_uuid != brew_uuid {
                    tracing::debug!("CMCE: brew_uuid changed during speaker change ({} -> {})", old_uuid, brew_uuid);
                    call.origin = CallOrigin::Network { brew_uuid };
                }
            }

            // Extract values before mutable borrow ends
            let call_id_val = *call_id;
            let ts = call.ts;
            let usage = call.usage;

            // End the mutable borrow
            let _ = call;

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

            // Respond to Brew with existing call resources, we already ensured it is cleared for brew
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallReady {
                    brew_uuid,
                    call_id: call_id_val,
                    ts,
                    usage,
                }),
            });
            return;
        }

        // New network call - allocate circuit
        let circuit = match {
            let mut state = self.config.state_write();
            self.circuits.allocate_circuit_with_allocator(
                Direction::Both,
                CommunicationType::P2Mp,
                &mut state.timeslot_alloc,
                TimeslotOwner::Cmce,
            )
        } {
            Ok(c) => c.clone(),
            Err(err) => {
                tracing::warn!("CMCE: failed to allocate circuit for network call: {:?}", err);
                return;
            }
        };

        let call_id = circuit.call_id;
        let ts = circuit.ts;
        let usage = circuit.usage;

        tracing::info!(
            "CMCE: starting NEW network call brew_uuid={} gssi={} speaker={} ts={} call_id={}",
            brew_uuid,
            dest_gssi,
            source_issi,
            ts,
            call_id
        );

        // Signal UMAC to open DL and UL circuits
        Self::signal_umac_circuit_open(queue, &circuit, None, CircuitDlMediaSource::LocalLoopback);

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

        // Cache for late-entry re-sends. Receipt starts as None so the CircuitMgr-triggered
        // backup send (within D_SETUP_REPEATS frames) is not throttled by this initial send.
        // The first re-send via tick_start will create a tracked receipt.
        self.cached_setups.insert(call_id, (d_setup, dest_addr, None));
        let (d_setup_ref, _, _) = self.cached_setups.get(&call_id).unwrap();

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

        // Track the active call
        self.active_calls.insert(
            call_id,
            ActiveCall {
                origin: CallOrigin::Network { brew_uuid },
                dest_gssi,
                source_issi,
                ts,
                usage,
                tx_active: true,
                hangtime_start: None,
                brew_uuid: Some(brew_uuid),
            },
        );

        // Respond to Brew with allocated resources, we already ensured it is cleared for brew
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Brew,
            msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallReady {
                brew_uuid,
                call_id,
                ts,
                usage,
            }),
        });
    }

    /// Handle network call end request
    fn rx_network_call_end(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid) {
        // Find the call by brew_uuid field (works for both Local and Network origin calls)
        let Some((call_id, call)) = self
            .active_calls
            .iter()
            .find(|(_, c)| c.brew_uuid == Some(brew_uuid))
            .map(|(id, c)| (*id, c.clone()))
        else {
            tracing::debug!("CMCE: network call end for unknown brew_uuid={}", brew_uuid);
            return;
        };

        tracing::info!(
            "CMCE: network call ended brew_uuid={} call_id={} gssi={}",
            brew_uuid,
            call_id,
            call.dest_gssi
        );

        // If currently transmitting, enter hangtime instead of immediate release
        let tx_active = call.tx_active;
        let dest_gssi = call.dest_gssi;
        let ts = call.ts;

        if tx_active {
            if let Some(active_call) = self.active_calls.get_mut(&call_id) {
                active_call.tx_active = false;
                active_call.hangtime_start = Some(self.dltime);
                active_call.brew_uuid = None;
            }
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
            .individual_calls
            .iter()
            .find(|(_, c)| c.ts == ts && c.state == IndividualCallState::Active && c.floor_holder.is_some())
            .map(|(id, _)| *id)
        {
            tracing::warn!("UL inactivity timeout on ts={}, releasing floor for individual call_id={}", ts, id);
            self.individual_tx_ceased(queue, id);
            return;
        }

        // Find the active call on this timeslot with tx_active == true
        let call_entry = self
            .active_calls
            .iter()
            .find(|(_, call)| call.ts == ts && call.tx_active)
            .map(|(id, _)| *id);

        let Some(call_id) = call_entry else {
            tracing::debug!("UL inactivity timeout on ts={} but no active transmitting call found", ts);
            return;
        };

        let call = self.active_calls.get_mut(&call_id).unwrap();
        tracing::warn!("UL inactivity timeout on ts={}, forcing TX ceased for call_id={}", ts, call_id);

        let dest_gssi = call.dest_gssi;
        call.tx_active = false;
        call.hangtime_start = Some(self.dltime);

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
        if net_brew::is_brew_gssi_routable(&self.config, dest_gssi) {
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
            });
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
