//! Brew protocol entity bridging TetraPack WebSocket to UMAC/MLE with hangtime-based circuit reuse

use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use uuid::Uuid;

use tetra_config::{SharedConfig, TimeslotOwner};
use tetra_core::{BitBuffer, Direction, Sap, SsiType, TdmaTime, TetraAddress, tetra_entities::TetraEntity};
use tetra_pdus::cmce::{
    enums::{call_timeout::CallTimeout, transmission_grant::TransmissionGrant},
    fields::basic_service_information::BasicServiceInformation,
    pdus::{d_connect::DConnect, d_release::DRelease, d_setup::DSetup, d_tx_ceased::DTxCeased, d_tx_granted::DTxGranted},
};
use tetra_saps::{
    SapMsg, SapMsgInner,
    control::brew::{BrewSubscriberAction, BrewSubscriberUpdate},
    control::{
        call_control::{CallControl, Circuit},
        enums::circuit_mode_type::CircuitModeType,
    },
    lcmc::{
        LcmcMleUnitdataReq,
        enums::{alloc_type::ChanAllocType, ul_dl_assignment::UlDlAssignment},
        fields::chan_alloc_req::CmceChanAllocReq,
    },
    tmd::TmdCircuitDataReq,
};

use crate::{MessageQueue, TetraEntityTrait};

use super::worker::{BrewCommand, BrewConfig, BrewEvent, BrewWorker};

/// Hangtime before releasing group call circuit to allow reuse without re-signaling.
const GROUP_CALL_HANGTIME: Duration = Duration::from_secs(1);

/// Active call tracking - Tracks the state of a single active Brew group call (currently transmitting)
#[derive(Debug)]
struct ActiveCall {
    /// Brew session UUID
    uuid: Uuid,
    /// TETRA call identifier (14-bit)
    call_id: u16,
    /// Allocated timeslot (2-4)
    ts: u8,
    /// Usage number for the channel allocation
    usage: u8,
    /// Calling party ISSI (from Brew)
    source_issi: u32,
    /// Destination GSSI (from Brew)
    dest_gssi: u32,
    /// Number of voice frames received
    frame_count: u64,
}

/// Group call in hangtime with circuit still allocated.
#[derive(Debug)]
struct HangingCall {
    /// TETRA call identifier (14-bit)
    call_id: u16,
    /// Allocated timeslot (2-4)
    ts: u8,
    /// Usage number for the channel allocation
    usage: u8,
    /// Last calling party ISSI (needed for D-SETUP re-send during late entry)
    source_issi: u32,
    /// Destination GSSI
    dest_gssi: u32,
    /// Total voice frames received during the call
    frame_count: u64,
    /// When the call entered hangtime (wall clock)
    since: Instant,
}

/// Tracks a local UL call being forwarded to TetraPack
#[derive(Debug)]
struct UlForwardedCall {
    /// Brew session UUID for this forwarded call
    uuid: Uuid,
    /// TETRA call identifier
    call_id: u16,
    /// Source ISSI of the calling radio
    source_issi: u32,
    /// Destination GSSI
    dest_gssi: u32,
    /// Number of voice frames forwarded
    frame_count: u64,
}

// ─── BrewEntity ───────────────────────────────────────────────────

pub struct BrewEntity {
    config: SharedConfig,
    dltime: TdmaTime,

    /// Receive events from the worker thread
    event_receiver: Receiver<BrewEvent>,
    /// Send commands to the worker thread
    command_sender: Sender<BrewCommand>,

    /// Active DL calls from Brew keyed by session UUID (currently transmitting)
    active_calls: HashMap<Uuid, ActiveCall>,

    /// DL calls in hangtime keyed by dest_gssi — circuit stays open, waiting for
    /// new speaker or timeout. Only one hanging call per GSSI.
    hanging_calls: HashMap<u32, HangingCall>,

    /// UL calls being forwarded to TetraPack, keyed by timeslot
    ul_forwarded: HashMap<u8, UlForwardedCall>,

    /// Registered subscriber groups (ISSI -> set of GSSIs)
    subscriber_groups: HashMap<u32, HashSet<u32>>,
    /// Listener counts per GSSI
    group_listeners: HashMap<u32, usize>,

    /// Circuit allocation state
    next_call_id: u16,
    next_usage: u8,

    /// Whether the worker is connected
    connected: bool,
}

impl BrewEntity {
    pub fn new(config: SharedConfig, brew_config: BrewConfig) -> Self {
        // Create channels
        let (event_sender, event_receiver) = unbounded::<BrewEvent>();
        let (command_sender, command_receiver) = unbounded::<BrewCommand>();

        // Spawn worker thread
        let worker_config = brew_config;
        thread::Builder::new()
            .name("brew-worker".to_string())
            .spawn(move || {
                let mut worker = BrewWorker::new(worker_config, event_sender, command_receiver);
                worker.run();
            })
            .expect("failed to spawn BrewWorker thread");

        Self {
            config,
            dltime: TdmaTime::default(),
            event_receiver,
            command_sender,
            active_calls: HashMap::new(),
            hanging_calls: HashMap::new(),
            ul_forwarded: HashMap::new(),
            subscriber_groups: HashMap::new(),
            group_listeners: HashMap::new(),
            next_call_id: 100, // Start at 100 to avoid collision with CMCE
            next_usage: 10,    // Start at 10 to avoid collision
            connected: false,
        }
    }

    /// Allocate a free timeslot for a new call. Returns (timeslot, call_id, usage) or None.
    fn allocate_timeslot(&mut self) -> Option<(u8, u16, u8)> {
        let ts = {
            let mut state = self.config.state_write();
            state.timeslot_alloc.allocate_any(TimeslotOwner::Brew)?
        };

        let call_id = self.next_call_id;
        self.next_call_id = if self.next_call_id >= 0x3FF { 100 } else { self.next_call_id + 1 };

        let usage = self.next_usage;
        self.next_usage = if self.next_usage >= 63 { 10 } else { self.next_usage + 1 };

        Some((ts, call_id, usage))
    }

    /// Release a timeslot
    fn release_timeslot(&mut self, ts: u8) {
        let mut state = self.config.state_write();
        if let Err(err) = state.timeslot_alloc.release(TimeslotOwner::Brew, ts) {
            tracing::warn!("BrewEntity: failed to release timeslot ts={} err={:?}", ts, err);
        }
    }

    /// Process all pending events from the worker thread
    fn process_events(&mut self, queue: &mut MessageQueue) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                BrewEvent::Connected => {
                    tracing::info!("BrewEntity: connected to TetraPack server");
                    self.connected = true;
                    self.resync_subscribers();
                }
                BrewEvent::Disconnected(reason) => {
                    tracing::warn!("BrewEntity: disconnected: {}", reason);
                    self.connected = false;
                    // Release all active calls
                    self.release_all_calls(queue);
                }
                BrewEvent::GroupCallStart {
                    uuid,
                    source_issi,
                    dest_gssi,
                    priority,
                    service,
                } => {
                    tracing::info!("BrewEntity: GROUP_TX service={} (0=TETRA ACELP, expect 0)", service);
                    self.handle_group_call_start(queue, uuid, source_issi, dest_gssi, priority);
                }
                BrewEvent::GroupCallEnd { uuid, cause } => {
                    self.handle_group_call_end(queue, uuid, cause);
                }
                BrewEvent::VoiceFrame { uuid, length_bits, data } => {
                    self.handle_voice_frame(queue, uuid, length_bits, data);
                }
                BrewEvent::SubscriberEvent { msg_type, issi, groups } => {
                    tracing::debug!("BrewEntity: subscriber event type={} issi={} groups={:?}", msg_type, issi, groups);
                }
                BrewEvent::ServerError { error_type, data } => {
                    let ascii = String::from_utf8_lossy(&data);
                    let trimmed = ascii.trim_matches('\0');
                    tracing::error!(
                        "BrewEntity: server error type={} data={} bytes ascii='{}'",
                        error_type,
                        data.len(),
                        trimmed
                    );
                    if trimmed.is_empty() {
                        tracing::error!("BrewEntity: server error raw={:02x?}", data);
                    }
                }
            }
        }
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

        let active_uuids: Vec<Uuid> = self
            .active_calls
            .iter()
            .filter(|(_, call)| call.dest_gssi == gssi)
            .map(|(uuid, _)| *uuid)
            .collect();

        for uuid in active_uuids {
            if let Some(call) = self.active_calls.remove(&uuid) {
                tracing::info!("BrewEntity: dropping active call uuid={} gssi={} (no listeners)", uuid, gssi);
                self.send_d_tx_ceased(queue, call.call_id, call.dest_gssi);
                self.finalize_call(queue, call.call_id, call.ts, call.dest_gssi);
            }
        }

        if let Some(hanging) = self.hanging_calls.remove(&gssi) {
            tracing::info!("BrewEntity: dropping hanging call gssi={} ts={} (no listeners)", gssi, hanging.ts);
            self.finalize_call(queue, hanging.call_id, hanging.ts, hanging.dest_gssi);
        }
    }

    fn handle_subscriber_update(&mut self, queue: &mut MessageQueue, update: BrewSubscriberUpdate) {
        let issi = update.issi;
        let groups = update.groups;

        match update.action {
            BrewSubscriberAction::Register => {
                let known = self.subscriber_groups.contains_key(&issi);
                self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                tracing::info!("BrewEntity: subscriber register issi={} known={}", issi, known);
                let _ = self.command_sender.send(BrewCommand::RegisterSubscriber { issi });
            }
            BrewSubscriberAction::Deregister => {
                if let Some(existing) = self.subscriber_groups.remove(&issi) {
                    for gssi in existing {
                        self.dec_group_listener(gssi);
                        self.drop_group_calls_if_unlistened(queue, gssi);
                    }
                }
                tracing::info!("BrewEntity: subscriber deregister issi={}", issi);
                let _ = self.command_sender.send(BrewCommand::DeregisterSubscriber { issi });
            }
            BrewSubscriberAction::Affiliate => {
                let is_new = !self.subscriber_groups.contains_key(&issi);
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

                if is_new {
                    tracing::info!("BrewEntity: affiliate from unknown issi={}, sending register", issi);
                    let _ = self.command_sender.send(BrewCommand::RegisterSubscriber { issi });
                }

                if new_groups.is_empty() {
                    tracing::debug!("BrewEntity: affiliate ignored (no new groups) issi={}", issi);
                } else {
                    tracing::info!("BrewEntity: subscriber affiliate issi={} groups={:?}", issi, new_groups);
                    let _ = self.command_sender.send(BrewCommand::AffiliateGroups { issi, groups: new_groups });
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
                    tracing::debug!("BrewEntity: deaffiliate ignored (no matching groups) issi={}", issi);
                } else {
                    tracing::info!("BrewEntity: subscriber deaffiliate issi={} groups={:?}", issi, removed_groups);
                    for gssi in &removed_groups {
                        self.drop_group_calls_if_unlistened(queue, *gssi);
                    }
                    let _ = self.command_sender.send(BrewCommand::DeaffiliateGroups {
                        issi,
                        groups: removed_groups,
                    });
                }
            }
        }
    }

    fn resync_subscribers(&self) {
        if self.subscriber_groups.is_empty() {
            tracing::debug!("BrewEntity: no subscribers to resync");
            return;
        }

        tracing::info!("BrewEntity: resyncing {} subscribers to TetraPack", self.subscriber_groups.len());

        for (issi, groups) in &self.subscriber_groups {
            let _ = self.command_sender.send(BrewCommand::RegisterSubscriber { issi: *issi });
            if !groups.is_empty() {
                let gssi_list: Vec<u32> = groups.iter().copied().collect();
                let _ = self.command_sender.send(BrewCommand::AffiliateGroups {
                    issi: *issi,
                    groups: gssi_list,
                });
            }
        }
    }

    /// Handle new group call from Brew, reusing hanging call circuits if available.
    fn handle_group_call_start(&mut self, queue: &mut MessageQueue, uuid: Uuid, source_issi: u32, dest_gssi: u32, _priority: u8) {
        if !self.has_listener(dest_gssi) {
            tracing::info!("BrewEntity: ignoring GROUP_TX uuid={} gssi={} (no listeners)", uuid, dest_gssi);
            return;
        }

        // Check if this call is already active (e.g., speaker change within same call)
        if let Some(call) = self.active_calls.get_mut(&uuid) {
            tracing::info!(
                "BrewEntity: GROUP_TX update on uuid={} new_src={} (was {})",
                uuid,
                source_issi,
                call.source_issi
            );
            call.source_issi = source_issi;
            return;
        }

        // Check if there's a hanging call for this GSSI — reuse the circuit
        if let Some(hanging) = self.hanging_calls.remove(&dest_gssi) {
            tracing::info!(
                "BrewEntity: reusing hanging circuit for gssi={} ts={} call_id={} (hangtime {:.1}s)",
                dest_gssi,
                hanging.ts,
                hanging.call_id,
                hanging.since.elapsed().as_secs_f32()
            );

            let call_id = hanging.call_id;
            let call = ActiveCall {
                uuid,
                call_id,
                ts: hanging.ts,
                usage: hanging.usage,
                source_issi,
                dest_gssi,
                frame_count: hanging.frame_count,
            };
            self.active_calls.insert(uuid, call);

            // Send D-TX GRANTED to reactivate MS U-plane for new speaker (EN 300 392-2, §14.5.2.2.1b).
            self.send_d_tx_granted(queue, call_id, source_issi, dest_gssi);
            return;
        }

        // New call — allocate a timeslot
        let Some((ts, call_id, usage)) = self.allocate_timeslot() else {
            tracing::warn!("BrewEntity: no free timeslot for group call uuid={} gssi={}", uuid, dest_gssi);
            return;
        };

        tracing::info!(
            "BrewEntity: starting group call uuid={} src={} gssi={} ts={} call_id={}",
            uuid,
            source_issi,
            dest_gssi,
            ts,
            call_id
        );

        // Track the call
        let call = ActiveCall {
            uuid,
            call_id,
            ts,
            usage,
            source_issi,
            dest_gssi,
            frame_count: 0,
        };
        self.active_calls.insert(uuid, call);

        // 1. Signal UMAC to open DL and UL circuits on this timeslot.
        let dl_circuit = Circuit {
            direction: Direction::Dl,
            ts,
            usage,
            circuit_mode: CircuitModeType::TchS,
            speech_service: Some(0), // TETRA encoded speech
            etee_encrypted: false,
        };
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::Open(dl_circuit)),
        });

        let ul_circuit = Circuit {
            direction: Direction::Ul,
            ts,
            usage,
            circuit_mode: CircuitModeType::TchS,
            speech_service: Some(0),
            etee_encrypted: false,
        };
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::Open(ul_circuit)),
        });

        // 2. Build and send D-SETUP PDU to radios
        self.send_d_setup(
            queue,
            call_id,
            usage,
            ts,
            source_issi,
            dest_gssi,
            TransmissionGrant::GrantedToOtherUser,
        );

        // 3. Build and send D-CONNECT PDU to confirm call is connected
        self.send_d_connect(queue, call_id, dest_gssi);
    }

    /// Build and send a D-SETUP PDU for a group call.
    /// `grant` controls the transmission_grant field:
    ///   - `GrantedToOtherUser` during active transmission
    ///   - `NotGranted` during hangtime (no one holds the floor)
    fn send_d_setup(
        &self,
        queue: &mut MessageQueue,
        call_id: u16,
        usage: u8,
        ts: u8,
        source_issi: u32,
        dest_gssi: u32,
        grant: TransmissionGrant,
    ) {
        let d_setup = DSetup {
            call_identifier: call_id,
            call_time_out: CallTimeout::T5m, // 5 minutes
            hook_method_selection: false,
            simplex_duplex_selection: false, // Simplex
            basic_service_information: BasicServiceInformation {
                circuit_mode_type: CircuitModeType::TchS,
                encryption_flag: false,
                communication_type: tetra_saps::control::enums::communication_type::CommunicationType::P2Mp,
                slots_per_frame: None,
                speech_service: Some(0), // TETRA encoded speech
            },
            transmission_grant: grant,
            transmission_request_permission: true,
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

        tracing::trace!("BrewEntity: -> D-SETUP call_id={} gssi={} grant={:?}", call_id, dest_gssi, grant);

        let mut sdu = BitBuffer::new_autoexpand(80);
        if let Err(e) = d_setup.to_bitbuf(&mut sdu) {
            tracing::error!("BrewEntity: failed to serialize D-SETUP: {:?}", e);
            return;
        }
        sdu.seek(0);

        // Build channel allocation for the timeslot
        let mut timeslots = [false; 4];
        timeslots[ts as usize - 1] = true;

        let chan_alloc = CmceChanAllocReq {
            usage: Some(usage),
            alloc_type: ChanAllocType::Replace,
            carrier: None,
            timeslots,
            ul_dl_assigned: UlDlAssignment::Both,
        };

        // Send via LCMC SAP to MLE, addressed to the GSSI
        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Mle,
            dltime: self.dltime,
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
                chan_alloc: Some(chan_alloc),
                main_address: TetraAddress::new(dest_gssi, SsiType::Gssi),
            }),
        };
        queue.push_back(msg);
    }

    /// Send D-CONNECT on MCCH to move MSes to "Traffic Connected" state after D-SETUP.
    fn send_d_connect(&self, queue: &mut MessageQueue, call_id: u16, dest_gssi: u32) {
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

        tracing::info!("BrewEntity: -> D-CONNECT call_id={} gssi={}", call_id, dest_gssi);

        let mut sdu = BitBuffer::new_autoexpand(40);
        if let Err(e) = d_connect.to_bitbuf(&mut sdu) {
            tracing::error!("BrewEntity: failed to serialize D-CONNECT: {:?}", e);
            return;
        }
        sdu.seek(0);

        // Send via LCMC SAP to MLE, addressed to the GSSI (no channel allocation needed)
        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Mle,
            dltime: self.dltime,
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
                main_address: TetraAddress::new(dest_gssi, SsiType::Gssi),
            }),
        };
        queue.push_back(msg);
    }

    /// Handle GROUP_IDLE by sending D-TX CEASED and entering hangtime instead of immediate teardown.
    fn handle_group_call_end(&mut self, queue: &mut MessageQueue, uuid: Uuid, _cause: u8) {
        let Some(call) = self.active_calls.remove(&uuid) else {
            tracing::debug!("BrewEntity: GROUP_IDLE for unknown uuid={} (already closed?)", uuid);
            return;
        };

        tracing::info!(
            "BrewEntity: GROUP_IDLE uuid={} gssi={} ts={} frames={} → entering hangtime ({:.0}s)",
            uuid,
            call.dest_gssi,
            call.ts,
            call.frame_count,
            GROUP_CALL_HANGTIME.as_secs_f32()
        );

        // Send D-TX CEASED to signal end of current transmission (EN 300 392-2, 14.7.1.13)
        self.send_d_tx_ceased(queue, call.call_id, call.dest_gssi);

        // Finalize any existing hanging call for this GSSI before replacing.
        if let Some(old) = self.hanging_calls.remove(&call.dest_gssi) {
            tracing::warn!(
                "BrewEntity: replacing existing hanging call for gssi={} ts={}",
                old.dest_gssi,
                old.ts
            );
            self.finalize_call(queue, old.call_id, old.ts, old.dest_gssi);
        }

        // Move to hanging state — circuit stays open
        self.hanging_calls.insert(
            call.dest_gssi,
            HangingCall {
                call_id: call.call_id,
                ts: call.ts,
                usage: call.usage,
                source_issi: call.source_issi,
                dest_gssi: call.dest_gssi,
                frame_count: call.frame_count,
                since: Instant::now(),
            },
        );
    }

    /// Expire hanging calls that have exceeded hangtime (called on each tick).
    fn expire_hanging_calls(&mut self, queue: &mut MessageQueue) {
        // Collect expired GSSIs first to avoid borrowing issues
        let expired: Vec<u32> = self
            .hanging_calls
            .iter()
            .filter(|(_, h)| h.since.elapsed() >= GROUP_CALL_HANGTIME)
            .map(|(&gssi, _)| gssi)
            .collect();

        for gssi in expired {
            let hanging = self.hanging_calls.remove(&gssi).unwrap();
            tracing::info!(
                "BrewEntity: hangtime expired for gssi={} ts={} call_id={} ({:.1}s elapsed, {} frames total)",
                gssi,
                hanging.ts,
                hanging.call_id,
                hanging.since.elapsed().as_secs_f32(),
                hanging.frame_count
            );
            self.finalize_call(queue, hanging.call_id, hanging.ts, hanging.dest_gssi);
        }
    }

    /// Finalize a call: send D-RELEASE, close circuits, release timeslot.
    fn finalize_call(&mut self, queue: &mut MessageQueue, call_id: u16, ts: u8, dest_gssi: u32) {
        // Send D-RELEASE (Brew cause 0 → TETRA cause 13 = Expiry_of_timer)
        self.send_d_release(queue, call_id, 13, dest_gssi);

        // Close both DL and UL circuits in UMAC
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::Close(Direction::Dl, ts)),
        });
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::Close(Direction::Ul, ts)),
        });

        // Free the timeslot
        self.release_timeslot(ts);
    }

    /// Send D-TX CEASED via FACCH/STCH to signal transmission end.
    fn send_d_tx_ceased(&self, queue: &mut MessageQueue, call_id: u16, dest_gssi: u32) {
        let d_tx_ceased = DTxCeased {
            call_identifier: call_id,
            transmission_request_permission: true,
            notification_indicator: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        tracing::info!("BrewEntity: -> D-TX CEASED call_id={} gssi={} (via FACCH)", call_id, dest_gssi);

        let mut sdu = BitBuffer::new_autoexpand(32);
        if let Err(e) = d_tx_ceased.to_bitbuf(&mut sdu) {
            tracing::error!("BrewEntity: failed to serialize D-TX CEASED: {:?}", e);
            return;
        }
        sdu.seek(0);

        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Mle,
            dltime: self.dltime,
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
                chan_alloc: None,
                main_address: TetraAddress::new(dest_gssi, SsiType::Gssi),
            }),
        };
        queue.push_back(msg);
    }

    /// Send D-TX GRANTED to group via FACCH/STCH to activate MS U-plane (EN 300 392-2, §14.5.2.2.1b).
    fn send_d_tx_granted(&self, queue: &mut MessageQueue, call_id: u16, source_issi: u32, dest_gssi: u32) {
        let d_tx_granted = DTxGranted {
            call_identifier: call_id,
            transmission_grant: TransmissionGrant::GrantedToOtherUser.into_raw() as u8,
            transmission_request_permission: false, // Other MSs should not request while someone is transmitting
            encryption_control: false,
            reserved: false,
            notification_indicator: None,
            transmitting_party_type_identifier: Some(1), // SSI present
            transmitting_party_address_ssi: Some(source_issi as u64),
            transmitting_party_extension: None,
            external_subscriber_number: None,
            facility: None,
            dm_ms_address: None,
            proprietary: None,
        };

        tracing::info!(
            "BrewEntity: -> D-TX GRANTED call_id={} gssi={} src={} (via FACCH)",
            call_id,
            dest_gssi,
            source_issi
        );

        let mut sdu = BitBuffer::new_autoexpand(64);
        if let Err(e) = d_tx_granted.to_bitbuf(&mut sdu) {
            tracing::error!("BrewEntity: failed to serialize D-TX GRANTED: {:?}", e);
            return;
        }
        sdu.seek(0);

        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Mle,
            dltime: self.dltime,
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
                chan_alloc: None,
                main_address: TetraAddress::new(dest_gssi, SsiType::Gssi),
            }),
        };
        queue.push_back(msg);
    }

    /// Build and send a D-RELEASE PDU
    fn send_d_release(&self, queue: &mut MessageQueue, call_id: u16, cause: u8, dest_gssi: u32) {
        let d_release = DRelease {
            call_identifier: call_id,
            disconnect_cause: cause,
            notification_indicator: None,
            facility: None,
            proprietary: None,
        };

        tracing::info!("BrewEntity: -> D-RELEASE call_id={} cause={}", call_id, cause);

        let mut sdu = BitBuffer::new_autoexpand(32);
        if let Err(e) = d_release.to_bitbuf(&mut sdu) {
            tracing::error!("BrewEntity: failed to serialize D-RELEASE: {:?}", e);
            return;
        }
        sdu.seek(0);

        let msg = SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Mle,
            dltime: self.dltime,
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
                main_address: TetraAddress::new(dest_gssi, SsiType::Gssi),
            }),
        };
        queue.push_back(msg);
    }

    /// Handle a voice frame from Brew — inject into the downlink
    fn handle_voice_frame(&mut self, queue: &mut MessageQueue, uuid: Uuid, _length_bits: u16, data: Vec<u8>) {
        let Some(call) = self.active_calls.get_mut(&uuid) else {
            // Voice frame for unknown call — might arrive before GROUP_TX or after GROUP_IDLE
            tracing::trace!("BrewEntity: voice frame for unknown uuid={} ({} bytes)", uuid, data.len());
            return;
        };

        call.frame_count += 1;

        // Log first voice frame per call
        if call.frame_count == 1 {
            tracing::info!(
                "BrewEntity: voice frame #{} uuid={} len={} bytes",
                call.frame_count,
                uuid,
                data.len()
            );
        }

        // STE format: byte 0 = header (control bits), bytes 1-35 = 274 ACELP bits for TCH/S.
        // Strip the STE header and pass only the ACELP payload.
        if data.len() < 36 {
            tracing::warn!("BrewEntity: voice frame too short ({} bytes, expected 36 STE bytes)", data.len());
            return;
        }
        let acelp_data = data[1..].to_vec(); // 35 bytes = 280 bits, of which 274 are ACELP

        // Inject ACELP frame into the downlink via TMD SAP
        let tmd_msg = SapMsg {
            sap: Sap::TmdSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::TmdCircuitDataReq(TmdCircuitDataReq {
                ts: call.ts,
                data: acelp_data,
            }),
        };
        queue.push_back(tmd_msg);
    }

    /// Send backup D-SETUP on MCCH for late entry support. Repetition rate is implementation-dependent (clause 23.0).
    /// Provides call details (call_id, GSSI, channel) so late-entering MSs can join ongoing calls (EN 300 392-2, Annex D).
    /// We send every 20th multiframe (~5.1s) to balance MCCH overhead and join latency.
    fn resend_d_setups_for_late_entry(&self, queue: &mut MessageQueue) {
        // Respect the late_entry_supported config setting
        if !self.config.config().cell.late_entry_supported {
            return;
        }

        // Only on TS1, frame 1, every 20th multiframe
        if self.dltime.t != 1 || self.dltime.f != 1 || self.dltime.m % 20 != 0 {
            return;
        }

        // Re-send D-SETUP for all active calls (someone is transmitting)
        for call in self.active_calls.values() {
            self.send_d_setup(
                queue,
                call.call_id,
                call.usage,
                call.ts,
                call.source_issi,
                call.dest_gssi,
                TransmissionGrant::GrantedToOtherUser,
            );
        }

        // Re-send D-SETUP for all hanging calls (circuit open, no one transmitting)
        for hanging in self.hanging_calls.values() {
            self.send_d_setup(
                queue,
                hanging.call_id,
                hanging.usage,
                hanging.ts,
                hanging.source_issi,
                hanging.dest_gssi,
                TransmissionGrant::NotGranted,
            );
        }
    }

    /// Release all active calls (on disconnect)
    fn release_all_calls(&mut self, queue: &mut MessageQueue) {
        // Finalize all active calls immediately (no hangtime on disconnect)
        let calls: Vec<(Uuid, ActiveCall)> = self.active_calls.drain().collect();
        for (_, call) in calls {
            self.send_d_tx_ceased(queue, call.call_id, call.dest_gssi);
            self.finalize_call(queue, call.call_id, call.ts, call.dest_gssi);
        }

        // Also finalize all hanging calls
        let hanging: Vec<(u32, HangingCall)> = self.hanging_calls.drain().collect();
        for (_, h) in hanging {
            self.finalize_call(queue, h.call_id, h.ts, h.dest_gssi);
        }
    }
}

// ─── TetraEntityTrait implementation ──────────────────────────────

impl TetraEntityTrait for BrewEntity {
    fn entity(&self) -> TetraEntity {
        TetraEntity::Brew
    }

    fn set_config(&mut self, config: SharedConfig) {
        self.config = config;
    }

    fn tick_start(&mut self, queue: &mut MessageQueue, ts: TdmaTime) {
        self.dltime = ts;
        // Process all pending events from the worker thread
        self.process_events(queue);
        // Expire hanging calls that have exceeded hangtime
        self.expire_hanging_calls(queue);
        // Periodically re-send D-SETUP for late entry support (EN 300 392-2 §14.5.1.3.2)
        self.resend_d_setups_for_late_entry(queue);
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
        match message.msg {
            // UL voice from UMAC — forward to TetraPack if this timeslot is being forwarded
            SapMsgInner::TmdCircuitDataInd(prim) => {
                self.handle_ul_voice(prim.ts, prim.data);
            }
            // Local call lifecycle notifications from CMCE
            SapMsgInner::CmceCallControl(CallControl::LocalCallStart {
                call_id,
                source_issi,
                dest_gssi,
                ts,
            }) => {
                self.handle_local_call_start(call_id, source_issi, dest_gssi, ts);
            }
            SapMsgInner::CmceCallControl(CallControl::LocalCallEnd { call_id, ts }) => {
                self.handle_local_call_end(call_id, ts);
            }
            SapMsgInner::BrewSubscriberUpdate(update) => {
                self.handle_subscriber_update(queue, update);
            }
            _ => {
                tracing::debug!("BrewEntity: unexpected rx_prim from {:?} on {:?}", message.src, message.sap);
            }
        }
    }
}

// ─── UL call forwarding to TetraPack ──────────────────────────────

impl BrewEntity {
    /// Handle notification that a local UL group call has started.
    /// If the group is subscribed (in config.groups), start forwarding to TetraPack.
    fn handle_local_call_start(&mut self, call_id: u16, source_issi: u32, dest_gssi: u32, ts: u8) {
        if !self.connected {
            tracing::trace!("BrewEntity: not connected, ignoring local call start");
            return;
        }

        if dest_gssi == 9 {
            tracing::debug!(
                "BrewEntity: suppressing local call forwarding for TG 9 (call_id={} src={} ts={})",
                call_id,
                source_issi,
                ts
            );
            return;
        }

        // If we're already forwarding on this timeslot, treat as a talker change/update
        if let Some(fwd) = self.ul_forwarded.get_mut(&ts) {
            if fwd.call_id != call_id || fwd.dest_gssi != dest_gssi {
                tracing::warn!(
                    "BrewEntity: updating forwarded call on ts={} (was call_id={} gssi={}) -> (call_id={} gssi={})",
                    ts,
                    fwd.call_id,
                    fwd.dest_gssi,
                    call_id,
                    dest_gssi
                );
            }

            fwd.call_id = call_id;
            fwd.source_issi = source_issi;
            fwd.dest_gssi = dest_gssi;
            fwd.frame_count = 0;

            // Send GROUP_TX update for the new talker
            let _ = self.command_sender.send(BrewCommand::SendGroupTx {
                uuid: fwd.uuid,
                source_issi,
                dest_gssi,
                priority: 0,
                service: 0, // TETRA encoded speech
            });
            return;
        }

        // Generate a UUID for this Brew session
        let uuid = Uuid::new_v4();
        tracing::info!(
            "BrewEntity: forwarding local call to TetraPack: call_id={} src={} gssi={} ts={} uuid={}",
            call_id,
            source_issi,
            dest_gssi,
            ts,
            uuid
        );

        // Send GROUP_TX to TetraPack
        let _ = self.command_sender.send(BrewCommand::SendGroupTx {
            uuid,
            source_issi,
            dest_gssi,
            priority: 0,
            service: 0, // TETRA encoded speech
        });

        // Track this forwarded call
        self.ul_forwarded.insert(
            ts,
            UlForwardedCall {
                uuid,
                call_id,
                source_issi,
                dest_gssi,
                frame_count: 0,
            },
        );
    }

    /// Handle notification that a local UL call has ended.
    fn handle_local_call_end(&mut self, call_id: u16, ts: u8) {
        if let Some(fwd) = self.ul_forwarded.remove(&ts) {
            if fwd.call_id != call_id {
                tracing::warn!(
                    "BrewEntity: call_id mismatch on ts={}: expected {} got {}",
                    ts,
                    fwd.call_id,
                    call_id
                );
            }
            tracing::info!(
                "BrewEntity: local call ended, sending GROUP_IDLE to TetraPack: uuid={} frames={}",
                fwd.uuid,
                fwd.frame_count
            );
            let _ = self.command_sender.send(BrewCommand::SendGroupIdle {
                uuid: fwd.uuid,
                cause: 0, // Normal release
            });
        }
    }

    /// Handle UL voice data from UMAC. If the timeslot is being forwarded to TetraPack,
    /// convert to STE format and send.
    fn handle_ul_voice(&mut self, ts: u8, acelp_bits: Vec<u8>) {
        let Some(fwd) = self.ul_forwarded.get_mut(&ts) else {
            return; // Not forwarded to TetraPack
        };

        fwd.frame_count += 1;

        // Convert ACELP bits to STE format for TetraPack.
        // Supported inputs:
        //   - 274 bytes (1-bit-per-byte) → pack to 35 bytes + header
        //   - 35 bytes (already packed) → prepend header
        //   - 36 bytes (already STE with header) → send as-is
        let ste_data = if acelp_bits.len() == 36 {
            acelp_bits
        } else if acelp_bits.len() == 35 {
            let mut ste = Vec::with_capacity(36);
            ste.push(0x00); // STE header byte: normal speech frame
            ste.extend_from_slice(&acelp_bits);
            ste
        } else {
            if acelp_bits.len() < 274 {
                tracing::warn!("BrewEntity: UL voice too short: {} bits", acelp_bits.len());
                return;
            }

            // Pack 274 bits into bytes, MSB first, prepend STE header
            let mut ste = Vec::with_capacity(36);
            ste.push(0x00); // STE header byte: normal speech frame

            // Pack 274 bits (1-per-byte) into 35 bytes (280 bits, last 6 bits padded)
            for chunk_idx in 0..35 {
                let mut byte = 0u8;
                for bit in 0..8 {
                    let bit_idx = chunk_idx * 8 + bit;
                    if bit_idx < 274 {
                        byte |= (acelp_bits[bit_idx] & 1) << (7 - bit);
                    }
                }
                ste.push(byte);
            }
            ste
        };

        let _ = self.command_sender.send(BrewCommand::SendVoiceFrame {
            uuid: fwd.uuid,
            length_bits: (ste_data.len() * 8) as u16,
            data: ste_data,
        });
    }
}
