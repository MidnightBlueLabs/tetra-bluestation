use std::collections::HashMap;

use tetra_config::SharedConfig;
use tetra_core::TimeslotOwner;
use tetra_core::{BitBuffer, Direction, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity, unimplemented_log};
use tetra_pdus::cmce::{
    enums::{
        call_timeout::CallTimeout, call_timeout_setup_phase::CallTimeoutSetupPhase, cmce_pdu_type_ul::CmcePduTypeUl,
        transmission_grant::TransmissionGrant,
    },
    fields::basic_service_information::BasicServiceInformation,
    pdus::{
        d_call_proceeding::DCallProceeding, d_connect::DConnect, d_release::DRelease, d_setup::DSetup, d_tx_ceased::DTxCeased,
        d_tx_granted::DTxGranted, u_release::URelease, u_setup::USetup, u_tx_ceased::UTxCeased, u_tx_demand::UTxDemand,
    },
    structs::cmce_circuit::CmceCircuit,
};
use tetra_saps::{
    SapMsg, SapMsgInner,
    control::{
        call_control::{CallControl, Circuit},
        enums::{circuit_mode_type::CircuitModeType, communication_type::CommunicationType},
    },
    lcmc::{
        LcmcMleUnitdataReq,
        enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment},
        fields::chan_alloc_req::CmceChanAllocReq,
    },
};

use crate::{
    MessageQueue,
    cmce::components::circuit_mgr::{CircuitMgr, CircuitMgrCmd},
};

/// Clause 11 Call Control CMCE sub-entity
pub struct CcBsSubentity {
    config: SharedConfig,
    dltime: TdmaTime,
    /// Cached D-SETUP PDUs for late-entry re-sends: call_id -> (D-SETUP PDU, dest address)
    cached_setups: HashMap<u16, (DSetup, TetraAddress)>,
    circuits: CircuitMgr,
    /// Active group calls: call_id -> call info
    active_calls: HashMap<u16, ActiveCall>,
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
}

impl CcBsSubentity {
    pub fn new(config: SharedConfig) -> Self {
        CcBsSubentity {
            config,
            dltime: TdmaTime::default(),
            cached_setups: HashMap::new(),
            circuits: CircuitMgr::new(),
            active_calls: HashMap::new(),
        }
    }

    pub fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    pub fn run_call_test(&mut self, queue: &mut MessageQueue, dltime: TdmaTime) {
        tracing::error!("-------- Running call test -------");

        // Create a new circuit
        let circuit = match {
            let mut state = self.config.state_write();
            self.circuits.allocate_circuit_with_allocator(
                Direction::Dl,
                CommunicationType::P2Mp,
                &mut state.timeslot_alloc,
                TimeslotOwner::Cmce,
            )
        } {
            Ok(circuit) => circuit,
            Err(e) => {
                tracing::error!("Failed to allocate circuit for call test: {:?}", e);
                return;
            }
        };

        // Signal UMAC to setup the circuit
        Self::signal_umac_circuit_open(queue, &circuit, dltime);

        // Build D-SETUP PDU and send down the stack
        let dest_addr = TetraAddress::new(26, SsiType::Gssi);
        let pdu_d_setup = Self::build_d_setup_pdu_from_circuit(&circuit);
        self.cached_setups.insert(circuit.call_id, (pdu_d_setup, dest_addr));
        let (pdu_ref, _) = self.cached_setups.get(&circuit.call_id).unwrap();

        let (pdu, chan_alloc) = Self::build_d_setup_prim(pdu_ref, circuit.usage, circuit.ts, UlDlAssignment::Dl);
        let prim = Self::build_sapmsg(pdu, Some(chan_alloc), dltime, dest_addr);
        queue.push_back(prim);
    }

    fn build_d_setup_pdu_from_circuit(circuit: &CmceCircuit) -> DSetup {
        DSetup {
            call_identifier: circuit.call_id,
            call_time_out: CallTimeout::Infinite,
            hook_method_selection: false,
            simplex_duplex_selection: circuit.simplex_duplex,
            basic_service_information: BasicServiceInformation {
                circuit_mode_type: circuit.circuit_mode,
                encryption_flag: circuit.etee_encrypted,
                communication_type: circuit.comm_type,
                slots_per_frame: None,
                speech_service: Some(0),
            },
            transmission_grant: TransmissionGrant::NotGranted,
            transmission_request_permission: false,
            call_priority: 0,
            notification_indicator: None,
            temporary_address: None,
            calling_party_address_ssi: Some(2041234),
            calling_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        }
    }

    fn build_d_setup_prim(pdu: &DSetup, usage: u8, ts: u8, ul_dl: UlDlAssignment) -> (BitBuffer, CmceChanAllocReq) {
        tracing::debug!("-> {:?}", pdu);

        let mut sdu = BitBuffer::new_autoexpand(80);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DSetup");
        sdu.seek(0);

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

    fn build_sapmsg(sdu: BitBuffer, chan_alloc: Option<CmceChanAllocReq>, dltime: TdmaTime, address: TetraAddress) -> SapMsg {
        // Construct prim
        SapMsg {
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
                chan_alloc,
                main_address: address,
            }),
        }
    }

    fn build_sapmsg_stealing(sdu: BitBuffer, dltime: TdmaTime, address: TetraAddress, ts: u8) -> SapMsg {
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
            dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 0,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: true,
                stealing_repeats_flag: false,
                chan_alloc: Some(chan_alloc),
                main_address: address,
            }),
        }
    }

    fn build_d_release_from_d_setup(d_setup_pdu: &DSetup) -> BitBuffer {
        let pdu = DRelease {
            call_identifier: d_setup_pdu.call_identifier,
            disconnect_cause: 13, // todo fixme
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };
        tracing::info!("-> {:?}", pdu);

        let mut sdu = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DRelease");
        sdu.seek(0);
        sdu
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
        tracing::debug!("send_d_call_proceeding: -> {:?} sdu {}", pdu_response, sdu.dump_bin());

        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime: message.dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: prim.handle,
                endpoint_id: prim.endpoint_id,
                link_id: prim.link_id,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,

                chan_alloc: None,
                main_address: prim.received_tetra_address,
                // redundant_transmission: 1,
            }),
        };
        queue.push_back(msg);
    }

    fn send_d_connect(&mut self, queue: &mut MessageQueue, message: &SapMsg, pdu_request: &USetup, call_id: u16) {
        tracing::trace!("send_d_connect");

        let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {
            panic!()
        };

        let pdu_response = DConnect {
            call_identifier: call_id,
            call_time_out: CallTimeout::T30m,
            hook_method_selection: pdu_request.hook_method_selection,
            simplex_duplex_selection: pdu_request.simplex_duplex_selection,
            transmission_grant: TransmissionGrant::Granted,
            transmission_request_permission: false, // CHECKME an MS may not ask for transmit permission
            call_ownership: false,                  // Group call meaning: false = not a call owner
            call_priority: None,
            basic_service_information: None,
            temporary_address: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(30);
        pdu_response.to_bitbuf(&mut sdu).expect("Failed to serialize DConnect");
        sdu.seek(0);
        tracing::debug!("send_d_connect: -> {:?} sdu {}", pdu_response, sdu.dump_bin());

        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime: message.dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: prim.handle,
                endpoint_id: prim.endpoint_id,
                link_id: prim.link_id,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,

                chan_alloc: None,
                main_address: prim.received_tetra_address,
                // redundant_transmission: 1,
            }),
        };
        queue.push_back(msg);
    }

    // fn send_d_setup(&mut self, queue: &mut MessageQueue, message: &SapMsg, pdu_request: &USetup, call_id: u16, calling_party: TetraAddress) {
    //     tracing::trace!("send_d_setup");

    //     let SapMsgInner::LcmcMleUnitdataInd(prim) = &message.msg else {panic!()};

    //     let transmission_grant = match pdu_request.request_to_transmit_send_data {
    //         true => TransmissionGrant::Granted,
    //         false => TransmissionGrant::NotGranted,
    //     };

    //     let pdu_response = DSetup {
    //         call_identifier: call_id,
    //         call_time_out: CallTimeout::T5m,
    //         hook_method_selection: pdu_request.hook_method_selection,
    //         simplex_duplex_selection: pdu_request.simplex_duplex_selection,
    //         basic_service_information: pdu_request.basic_service_information.clone(),
    //         transmission_grant: transmission_grant,
    //         transmission_request_permission: false,
    //         call_priority: 0,
    //         temporary_address: None,
    //         calling_party_address_ssi: Some(calling_party.ssi),
    //         calling_party_extension: None,
    //         external_subscriber_number: None,
    //         dm_ms_address: None,
    //         notification_indicator: None,
    //         facility: None,
    //         proprietary: None,
    //     };

    //     let mut sdu = BitBuffer::new_autoexpand(71);
    //     pdu_response.to_bitbuf(&mut sdu).expect("Failed to serialize DSetup");
    //     sdu.seek(0);
    //     tracing::debug!("send_d_setup: -> {:?} sdu {}", pdu_response, sdu.dump_bin());

    //     let chan_alloc = Some(CmceChanAllocReq {
    //         usage: self.circuit_alloc_usage(),
    //         alloc_type: ChanAllocType::Replace,
    //         carrier: None,
    //         timeslots: [false, true, false, false],
    //         ul_dl_assigned: UlDlAssignment::Both,
    //     });

    //     let msg = SapMsg {
    //         sap: Sap::LcmcSap,
    //         src: TetraEntity::Cmce,
    //         dest: TetraEntity::Mle,
    //         dltime: message.dltime,
    //         msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq{
    //             sdu: sdu,
    //             handle: prim.handle,
    //             endpoint_id: prim.endpoint_id,
    //             link_id: prim.link_id,
    //             layer2service: 0,
    //             pdu_prio: 0,
    //             layer2_qos: 0,
    //             stealing_permission: false,
    //             stealing_repeats_flag: false,
    //             chan_alloc,
    //             main_address: prim.received_tetra_address,
    //             // redundant_transmission: 4,
    //         })
    //     };
    //     queue.push_back(msg);
    // }

    fn signal_umac_circuit_open(queue: &mut MessageQueue, call: &CmceCircuit, dltime: TdmaTime) {
        let circuit = Circuit {
            direction: call.direction,
            ts: call.ts,
            usage: call.usage,
            circuit_mode: call.circuit_mode,
            speech_service: call.speech_service,
            etee_encrypted: call.etee_encrypted,
        };
        let cmd = SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::Open(circuit)),
        };
        queue.push_back(cmd);
    }

    fn signal_umac_circuit_close(queue: &mut MessageQueue, circuit: CmceCircuit, dltime: TdmaTime) {
        let cmd = SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            dltime,
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
                tracing::debug!("<- U-SETUP {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-SETUP: {:?} {}", e, prim.sdu.dump_bin());
                return;
            }
        };

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
        Self::signal_umac_circuit_open(queue, &circuit, message.dltime);

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
            call_ownership: false,
            call_priority: None,
            basic_service_information: None,
            temporary_address: None,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        tracing::info!("-> {:?}", d_connect);
        let mut connect_sdu = BitBuffer::new_autoexpand(30);
        d_connect.to_bitbuf(&mut connect_sdu).expect("Failed to serialize DConnect");
        connect_sdu.seek(0);

        let connect_msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime: message.dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu: connect_sdu,
                handle: ul_handle,
                endpoint_id: ul_endpoint_id,
                link_id: ul_link_id,
                layer2service: 0,
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

        // Cache for late-entry re-sends
        self.cached_setups.insert(circuit.call_id, (d_setup, dest_addr));
        let (d_setup_ref, _) = self.cached_setups.get(&circuit.call_id).unwrap();

        let (setup_sdu, setup_chan_alloc) = Self::build_d_setup_prim(d_setup_ref, circuit.usage, circuit.ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, Some(setup_chan_alloc), message.dltime, dest_addr);
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
            },
        );

        // Notify Brew entity about this local call if Brew is loaded.
        // It can then forward to TetraPack if the group is subscribed
        if self.config.config().brew.is_some() {
            let msg = SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                dltime: message.dltime,
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
            CmcePduTypeUl::UAlert
            | CmcePduTypeUl::UConnect
            | CmcePduTypeUl::UDisconnect
            | CmcePduTypeUl::UInfo
            | CmcePduTypeUl::UStatus
            | CmcePduTypeUl::UCallRestore => {
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

        if let Some(tasks) = self.circuits.tick_start(dltime) {
            for task in tasks {
                match task {
                    CircuitMgrCmd::SendDSetup(call_id, usage, ts) => {
                        // Get our cached D-SETUP, build a prim and send it down the stack
                        let Some((pdu, dest_addr)) = self.cached_setups.get(&call_id) else {
                            tracing::error!("No cached D-SETUP for call id {}", call_id);
                            return;
                        };
                        let dest_addr = *dest_addr;
                        let (sdu, chan_alloc) = Self::build_d_setup_prim(pdu, usage, ts, UlDlAssignment::Both);
                        let prim = Self::build_sapmsg(sdu, Some(chan_alloc), self.dltime, dest_addr);
                        queue.push_back(prim);
                    }

                    CircuitMgrCmd::SendClose(call_id, circuit) => {
                        tracing::warn!("need to send CLOSE for call id {}", call_id);
                        let ts = circuit.ts;
                        // Get our cached D-SETUP, build D-RELEASE and send
                        if let Some((pdu, dest_addr)) = self.cached_setups.get(&call_id) {
                            let dest_addr = *dest_addr;
                            let sdu = Self::build_d_release_from_d_setup(pdu);
                            let prim = Self::build_sapmsg(sdu, None, self.dltime, dest_addr);
                            queue.push_back(prim);
                        } else {
                            tracing::error!("No cached D-SETUP for call id {}", call_id);
                        }

                        // Clean up call state
                        self.cached_setups.remove(&call_id);
                        self.active_calls.remove(&call_id);

                        // Signal UMAC to release the circuit
                        Self::signal_umac_circuit_close(queue, circuit, self.dltime);
                        self.release_timeslot(ts);
                    }
                }
            }
        }
    }

    /// Check if any active calls in hangtime have expired, and if so, release them
    fn check_hangtime_expiry(&mut self, queue: &mut MessageQueue) {
        // Hangtime: ~5 seconds = 5 * 18 * 4 = 360 frames (approximately)
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
            self.release_call(queue, call_id);
        }
    }

    fn release_timeslot(&mut self, ts: u8) {
        let mut state = self.config.state_write();
        if let Err(err) = state.timeslot_alloc.release(TimeslotOwner::Cmce, ts) {
            tracing::warn!("CcBsSubentity: failed to release timeslot ts={} err={:?}", ts, err);
        }
    }

    /// Release a call: send D-RELEASE, close circuits, clean up state
    fn release_call(&mut self, queue: &mut MessageQueue, call_id: u16) {
        let Some((pdu, dest_addr)) = self.cached_setups.get(&call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = *dest_addr;

        // Send D-RELEASE to group
        let sdu = Self::build_d_release_from_d_setup(pdu);
        let prim = Self::build_sapmsg(sdu, None, self.dltime, dest_addr);
        queue.push_back(prim);

        // Close the circuit in CircuitMgr and notify Brew
        if let Some(call) = self.active_calls.get(&call_id) {
            let ts = call.ts;
            let is_local = matches!(call.origin, CallOrigin::Local { .. });

            if let Ok(circuit) = self.circuits.close_circuit(Direction::Both, ts) {
                Self::signal_umac_circuit_close(queue, circuit, self.dltime);
            }

            // Ensure UMAC clears any hangtime override for this slot even if the circuit close is delayed.
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                dltime: self.dltime,
                msg: SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts }),
            });

            self.release_timeslot(ts);

            // Notify Brew only for local calls
            if self.config.config().brew.is_some() {
                if is_local {
                    let notify = SapMsg {
                        sap: Sap::Control,
                        src: TetraEntity::Cmce,
                        dest: TetraEntity::Brew,
                        dltime: self.dltime,
                        msg: SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts }),
                    };
                    queue.push_back(notify);
                }
            }
        }

        // Clean up
        self.cached_setups.remove(&call_id);
        self.active_calls.remove(&call_id);
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
                tracing::debug!("<- U-TX CEASED {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-TX CEASED: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;

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
        let is_local = matches!(call.origin, CallOrigin::Local { .. });
        call.tx_active = false;
        call.hangtime_start = Some(self.dltime);

        // Get dest address from cached setup
        let Some((_, dest_addr)) = self.cached_setups.get(&call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = *dest_addr;

        // Send D-TX CEASED via FACCH (stealing) to all group members
        let d_tx_ceased = DTxCeased {
            call_identifier: call_id,
            transmission_request_permission: true, // Allow other MSs to request the floor
            notification_indicator: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        tracing::info!("-> {:?}", d_tx_ceased);
        let mut sdu = BitBuffer::new_autoexpand(25);
        d_tx_ceased.to_bitbuf(&mut sdu).expect("Failed to serialize DTxCeased");
        sdu.seek(0);

        // Send via FACCH (stealing channel) so radios on the traffic channel hear the beep
        let msg = Self::build_sapmsg_stealing(sdu, self.dltime, dest_addr, ts);
        queue.push_back(msg);

        // Notify UMAC to enter hangtime signalling mode on this traffic timeslot.
        // This stops downlink TCH fill frames (zeros) and enables UL CommonAndAssigned so MS can request the floor.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
        });

        // Notify Brew to stop forwarding audio for local calls
        if self.config.config().brew.is_some() {
            if is_local {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    dltime: self.dltime,
                    msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
                });
            }
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
                tracing::debug!("<- U-TX DEMAND {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-TX DEMAND: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;

        let Some(call) = self.active_calls.get_mut(&call_id) else {
            tracing::warn!("U-TX DEMAND for unknown call_id={}", call_id);
            return;
        };

        tracing::info!("U-TX DEMAND: ISSI {} requests floor on call_id={}", requesting_party.ssi, call_id);

        // Grant the floor to the requesting MS
        let ts = call.ts;
        call.tx_active = true;
        call.hangtime_start = None;
        call.source_issi = requesting_party.ssi;

        // Update caller_addr for local calls
        if let CallOrigin::Local { caller_addr } = &mut call.origin {
            *caller_addr = requesting_party;
        }

        let Some((_, dest_addr)) = self.cached_setups.get(&call_id) else {
            tracing::error!("No cached D-SETUP for call_id={}", call_id);
            return;
        };
        let dest_addr = *dest_addr;

        // Send D-TX GRANTED via FACCH
        let d_tx_granted = DTxGranted {
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

        tracing::info!("-> {:?}", d_tx_granted);
        let mut sdu = BitBuffer::new_autoexpand(50);
        d_tx_granted.to_bitbuf(&mut sdu).expect("Failed to serialize DTxGranted");
        sdu.seek(0);

        let msg = Self::build_sapmsg_stealing(sdu, self.dltime, dest_addr, ts);
        queue.push_back(msg);

        // Notify UMAC to resume traffic mode (exit hangtime) for this timeslot.
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                call_id,
                source_issi: requesting_party.ssi,
                dest_gssi: dest_addr.ssi,
                ts,
            }),
        });

        // Notify Brew only for local calls (speaker change = new FloorGranted for new speaker)
        if self.config.config().brew.is_some() {
            let Some(call) = self.active_calls.get(&call_id) else {
                return;
            };
            if matches!(call.origin, CallOrigin::Local { .. }) {
                let notify = SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    dltime: self.dltime,
                    msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                        call_id,
                        source_issi: requesting_party.ssi,
                        dest_gssi: dest_addr.ssi,
                        ts: call.ts,
                    }),
                };
                queue.push_back(notify);
            }
        }
    }

    /// Handle U-RELEASE: radio explicitly releases the call
    fn rx_u_release(&mut self, queue: &mut MessageQueue, mut message: SapMsg) {
        let SapMsgInner::LcmcMleUnitdataInd(prim) = &mut message.msg else {
            panic!()
        };

        let pdu = match URelease::from_bitbuf(&mut prim.sdu) {
            Ok(pdu) => {
                tracing::debug!("<- U-RELEASE {:?}", pdu);
                pdu
            }
            Err(e) => {
                tracing::warn!("Failed parsing U-RELEASE: {:?}", e);
                return;
            }
        };

        let call_id = pdu.call_identifier;
        tracing::info!("U-RELEASE: call_id={} cause={}", call_id, pdu.disconnect_cause);
        self.release_call(queue, call_id);
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
            _ => {
                tracing::warn!("Unexpected CallControl message: {:?}", call_control);
            }
        }
    }

    /// Handle network-initiated group call start
    fn rx_network_call_start(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid, source_issi: u32, dest_gssi: u32, _priority: u8) {
        // Check if there's an active call for this GSSI (speaker change scenario)
        if let Some((call_id, call)) = self.active_calls.iter_mut().find(|(_, c)| c.dest_gssi == dest_gssi) {
            // Speaker change during active or hangtime
            tracing::info!(
                "CMCE: network call speaker change gssi={} new_speaker={} (was {})",
                dest_gssi,
                source_issi,
                call.source_issi
            );

            call.source_issi = source_issi;
            call.tx_active = true;
            call.hangtime_start = None;

            if let CallOrigin::Network { brew_uuid: old_uuid } = call.origin {
                // Update UUID if different (shouldn't happen but handle it)
                if old_uuid != brew_uuid {
                    tracing::warn!("CMCE: brew_uuid changed during speaker change");
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
                dltime: self.dltime,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                    call_id: call_id_val,
                    source_issi,
                    dest_gssi,
                    ts,
                }),
            });

            // Respond to Brew with existing call resources
            if self.config.config().brew.is_some() {
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Cmce,
                    dest: TetraEntity::Brew,
                    dltime: self.dltime,
                    msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallReady {
                        brew_uuid,
                        call_id: call_id_val,
                        ts,
                        usage,
                    }),
                });
            }
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
        Self::signal_umac_circuit_open(queue, &circuit, self.dltime);

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

        // Cache for late-entry re-sends
        self.cached_setups.insert(call_id, (d_setup, dest_addr.clone()));
        let (d_setup_ref, _) = self.cached_setups.get(&call_id).unwrap();

        let (setup_sdu, setup_chan_alloc) = Self::build_d_setup_prim(d_setup_ref, usage, ts, UlDlAssignment::Both);
        let setup_msg = Self::build_sapmsg(setup_sdu, Some(setup_chan_alloc), self.dltime, dest_addr.clone());
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
            dltime: self.dltime,
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu: connect_sdu,
                handle: 0, // Broadcast to group, no specific handle
                endpoint_id: 0,
                link_id: 0,
                layer2service: 0,
                pdu_prio: 0,
                layer2_qos: 0,
                stealing_permission: false,
                stealing_repeats_flag: false,
                chan_alloc: None, // Already sent in D-SETUP
                main_address: dest_addr,
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
            },
        );

        // Respond to Brew with allocated resources
        if self.config.config().brew.is_some() {
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Brew,
                dltime: self.dltime,
                msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallReady {
                    brew_uuid,
                    call_id,
                    ts,
                    usage,
                }),
            });
        }
    }

    /// Handle network call end request
    fn rx_network_call_end(&mut self, queue: &mut MessageQueue, brew_uuid: uuid::Uuid) {
        // Find the call by brew_uuid
        let Some((call_id, call)) = self
            .active_calls
            .iter()
            .find(|(_, c)| matches!(c.origin, CallOrigin::Network { brew_uuid: u } if u == brew_uuid))
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
            }
            // Send D-TX CEASED via FACCH
            self.send_d_tx_ceased_facch(queue, call_id, dest_gssi, ts);

            // Notify UMAC to enter hangtime signalling mode on this traffic timeslot.
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Cmce,
                dest: TetraEntity::Umac,
                dltime: self.dltime,
                msg: SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }),
            });
        } else {
            // Already in hangtime or idle, release immediately
            self.release_call(queue, call_id);
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

        tracing::debug!("-> D-TX GRANTED (FACCH) {:?}", pdu);
        let mut sdu = BitBuffer::new_autoexpand(30);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DTxGranted");
        sdu.seek(0);

        let dest_addr = TetraAddress::new(dest_gssi, SsiType::Gssi);
        let msg = Self::build_sapmsg_stealing(sdu, self.dltime, dest_addr, ts);
        queue.push_back(msg);
    }

    /// Send D-TX CEASED via FACCH stealing
    fn send_d_tx_ceased_facch(&mut self, queue: &mut MessageQueue, call_id: u16, dest_gssi: u32, ts: u8) {
        let pdu = DTxCeased {
            call_identifier: call_id,
            transmission_request_permission: true,
            notification_indicator: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        tracing::debug!("-> D-TX CEASED (FACCH) {:?}", pdu);
        let mut sdu = BitBuffer::new_autoexpand(30);
        pdu.to_bitbuf(&mut sdu).expect("Failed to serialize DTxCeased");
        sdu.seek(0);

        let dest_addr = TetraAddress::new(dest_gssi, SsiType::Gssi);
        let msg = Self::build_sapmsg_stealing(sdu, self.dltime, dest_addr, ts);
        queue.push_back(msg);
    }
}
