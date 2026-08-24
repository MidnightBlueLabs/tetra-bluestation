//! Brew protocol entity bridging a remote network backend to UMAC/MLE
//!
//! Call state lives entirely in the global circuit store: a call this entity bridges carries
//! the Brew session uuid on its downlink, so a signal naming that session is enough to look
//! up everything else.
//!
//! Transport-agnostic: the concrete transport (WebSocket, QUIC, TCP, …) is
//! injected at construction time via [`BrewEntity::new`].

use std::collections::{HashMap, HashSet};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};
use tetra_pdus::cmce::enums::transmission_grant::TransmissionGrant;
use tetra_saps::control::call_signal::{BrewEvent, CmceEvent};
use tetra_saps::control::enums::sds_user_data::SdsUserData;
use tetra_saps::control::sds::CmceSdsData;
use uuid::Uuid;

use crate::net_brew::components::jitter_buffer::VoiceJitterBuffer;
use crate::network::transports::NetworkTransport;
use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::{CfgBrew, CircuitStreamDest, NetworkCallRequest, SharedConfig, StackState, TetraCircuit};
use tetra_core::{Sap, TdmaTime, tetra_entities::TetraEntity};
use tetra_saps::control::brew::{BrewSubscriberAction, MmSubscriberUpdate};
use tetra_saps::{SapMsg, SapMsgInner, tmd::TmdCircuitDataReq};

use super::protocol::BrewCircularCall;
// The worker speaks the wire protocol, which is session based; the SAP event of the same name
// is the call level one signalled to CMCE.
use super::worker::{BrewCommand, BrewEvent as WorkerEvent, BrewWorker};

// ─── BrewEntity ───────────────────────────────────────────────────

pub struct BrewEntity {
    config: SharedConfig,
    state: StackState,

    /// Also contained in the SharedConfig, but kept for fast, convenient access
    brew_config: CfgBrew,

    dltime: TdmaTime,

    /// Receive events from the worker thread
    event_receiver: Receiver<WorkerEvent>,
    /// Send commands to the worker thread
    command_sender: Sender<BrewCommand>,

    /// Per-session jitter/playout buffer for downlink voice from the backend.
    dl_jitter: HashMap<Uuid, VoiceJitterBuffer>,

    /// Registered subscriber groups (ISSI -> set of GSSIs)
    subscriber_groups: HashMap<u32, HashSet<u32>>,

    /// Worker thread handle for graceful shutdown
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl BrewEntity {
    /// Create a new BrewEntity with the given transport.
    ///
    /// The transport is moved into a worker thread. Any [`NetworkTransport`]
    /// implementation can be used (WebSocket, QUIC, TCP, …).
    pub fn new<T: NetworkTransport + 'static>(config: SharedConfig, state: StackState, transport: T) -> Self {
        // Create channels
        let (event_sender, event_receiver) = unbounded::<WorkerEvent>();
        let (command_sender, command_receiver) = unbounded::<BrewCommand>();

        // Spawn worker thread with the provided transport
        let brew_config = config.config().as_ref().brew.clone().unwrap(); // Never fails
        let worker_config = config.clone();
        let handle = thread::Builder::new()
            .name("brew-worker".to_string())
            .spawn(move || {
                let mut worker = BrewWorker::new(worker_config, event_sender, command_receiver, transport);
                worker.run();
            })
            .expect("failed to spawn BrewWorker thread");

        Self {
            config,
            state,
            brew_config,
            dltime: TdmaTime::default(),
            event_receiver,
            command_sender,
            dl_jitter: HashMap::new(),
            subscriber_groups: HashMap::new(),
            worker_handle: Some(handle),
        }
    }

    /// The circuit a Brew session is attached to, while the call exists.
    fn circuit_by_uuid(&self, uuid: Uuid) -> Option<TetraCircuit> {
        self.state.with_circuits(|c| c.get_circuit_by_uuid(uuid).cloned())
    }

    fn circuit_by_callid(&self, call_id: u16) -> Option<TetraCircuit> {
        self.state.with_circuits(|c| c.get_circuit_by_callid(call_id).cloned())
    }

    /// The call a Brew session belongs to, once CMCE has a circuit for it.
    fn callid_by_uuid(&self, uuid: Uuid) -> Option<u16> {
        self.state.with_circuits(|c| c.get_callid_by_uuid(uuid))
    }

    /// The call a backend event refers to. Nothing can be done for a session without a call:
    /// the setup that would create one is still with CMCE, or the call is long gone.
    fn resolve_call(&self, uuid: Uuid, what: &str) -> Option<u16> {
        let call_id = self.callid_by_uuid(uuid);
        if call_id.is_none() {
            tracing::debug!("BrewEntity: {} for uuid={} without a call", what, uuid);
        }
        call_id
    }

    /// The Brew session carrying a call: the one on its circuit, or, once the call is torn
    /// down, the one CMCE handed over at teardown.
    fn session_of(&self, call_id: u16) -> Option<Uuid> {
        self.state.with_circuits(|c| {
            let live = c
                .get_circuit_by_callid(call_id)
                .and_then(|circuit| circuit.brew_uuid().or(circuit.brew_origin_uuid));
            match live {
                Some(uuid) => Some(uuid),
                None => c.take_closing_session(call_id),
            }
        })
    }

    /// The circuit whose calling party transmits on this timeslot.
    fn circuit_by_ul_ts(&self, ts: u8) -> Option<TetraCircuit> {
        self.state.with_circuits(|c| {
            let call_id = c.get_callid_by_ul_ts(ts)?;
            c.get_circuit_by_callid(call_id).cloned()
        })
    }

    /// A session is known while its call exists, or while its parameters still await pickup
    /// by CMCE (setup in flight).
    fn is_known_session(&self, uuid: Uuid) -> bool {
        self.state
            .with_circuits(|c| c.get_callid_by_uuid(uuid).is_some() || c.has_network_request(uuid))
    }

    /// Deposit the parameters of a call for CMCE to pick up with the accompanying signal.
    fn put_network_request(&self, uuid: Uuid, request: NetworkCallRequest) {
        self.state.with_circuits(|c| c.put_network_request(uuid, request));
    }

    /// Collect the parameters CMCE deposited for a signalled call.
    fn take_network_request(&self, uuid: Uuid) -> Option<NetworkCallRequest> {
        self.state.with_circuits(|c| c.take_network_request(uuid))
    }

    /// Drop a call the backend tore down before CMCE picked it up: the deposited parameters
    /// are all there is of it, so the call never starts.
    fn cancel_pending_call(&self, uuid: Uuid, what: &str) {
        if self.take_network_request(uuid).is_some() {
            tracing::info!("BrewEntity: {} uuid={} cancels a call CMCE has not picked up yet", what, uuid);
        } else {
            tracing::debug!("BrewEntity: {} for unknown uuid={}", what, uuid);
        }
    }

    /// Detach the Brew session from its call, leaving the circuit purely local. Used once the
    /// upstream session is over while the call itself may live on (hangtime, new speaker).
    fn detach_session(&self, uuid: Uuid) {
        self.state.with_circuits(|c| {
            let Some(call_id) = c.get_callid_by_uuid(uuid) else {
                return;
            };
            let Some(ts) = c.get_circuit_by_callid(call_id).map(|circuit| circuit.dl_ts()) else {
                return;
            };
            c.update_circuit_with(call_id, |circuit| circuit.dl1_source = CircuitStreamDest::Local(Some(ts)));
        });
    }

    /// Signal CMCE what the backend did to a call. The event names the call and nothing else:
    /// call parameters, where any exist, travel through the network call request store.
    fn signal_cmce(queue: &mut MessageQueue, event: BrewEvent) {
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Cmce,
            msg: SapMsgInner::BrewCallEvent(event),
        });
    }

    /// Playout buffer for a session, created on first use.
    fn jitter_buffer(&mut self, uuid: Uuid) -> &mut VoiceJitterBuffer {
        let latency = self.brew_config.jitter_initial_latency_frames as usize;
        self.dl_jitter
            .entry(uuid)
            .or_insert_with(|| VoiceJitterBuffer::with_initial_latency(latency))
    }

    /// Retrieves current connection status from global state
    fn state_is_connected(&self) -> bool {
        self.state.with_global_state(|x| x.network_connected)
    }

    /// Updates global state to reflect we are either connected or disconnected
    fn state_set_connected(&self, is_connected: bool) {
        self.state.with_global_state(|x| x.network_connected = is_connected);
        tracing::info!("BrewEntity: backhaul {}", if is_connected { "CONNECTED" } else { "DISCONNECTED" });
    }

    /// Process all pending events from the worker thread
    fn process_events(&mut self, queue: &mut MessageQueue) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                WorkerEvent::Connected => {
                    tracing::debug!("BrewEntity: connected to TetraPack server");
                    self.state_set_connected(true);
                    self.resync_subscribers();
                }
                WorkerEvent::Disconnected(reason) => {
                    tracing::debug!("BrewEntity: disconnected: {}", reason); // Already warned in worker
                    self.state_set_connected(false);
                    self.release_all_calls(queue);
                }
                WorkerEvent::GroupCallStart {
                    uuid,
                    source_issi,
                    dest_gssi,
                    priority,
                    service,
                } => {
                    tracing::info!("BrewEntity: GROUP_TX service={} (0=TETRA ACELP, expect 0)", service);
                    self.handle_group_call_start(queue, uuid, source_issi, dest_gssi, priority);
                }
                WorkerEvent::GroupCallEnd { uuid, cause } => {
                    self.handle_group_call_end(queue, uuid, cause);
                }
                WorkerEvent::CircuitSetupRequest { uuid, call } => {
                    // A call the backend offers has no circuit yet, so it is named by session.
                    self.put_network_request(uuid, Self::brew_to_network_request(&call));
                    Self::signal_cmce(queue, BrewEvent::SetupRequest { brew_uuid: uuid });
                }
                WorkerEvent::CircuitSetupAccept { uuid } => {
                    if let Some(call_id) = self.resolve_call(uuid, "SETUP_ACCEPT") {
                        Self::signal_cmce(queue, BrewEvent::SetupAccept { call_id });
                    }
                }
                WorkerEvent::CircuitSetupReject { uuid, cause } => {
                    self.dl_jitter.remove(&uuid);
                    match self.callid_by_uuid(uuid) {
                        Some(call_id) => Self::signal_cmce(queue, BrewEvent::SetupReject { call_id, cause }),
                        None => self.cancel_pending_call(uuid, "SETUP_REJECT"),
                    }
                }
                WorkerEvent::CircuitAlert { uuid } => {
                    if let Some(call_id) = self.resolve_call(uuid, "CALL_ALERT") {
                        Self::signal_cmce(queue, BrewEvent::Alert { call_id });
                    }
                }
                WorkerEvent::CircuitConnectRequest { uuid, call } => {
                    if let Some(call_id) = self.resolve_call(uuid, "CONNECT_REQUEST") {
                        self.put_network_request(uuid, Self::brew_to_network_request(&call));
                        Self::signal_cmce(queue, BrewEvent::ConnectRequest { call_id });
                    }
                }
                WorkerEvent::CircuitConnectConfirm { uuid, .. } => {
                    if let Some(call_id) = self.resolve_call(uuid, "CONNECT_CONFIRM") {
                        Self::signal_cmce(queue, BrewEvent::ConnectConfirm { call_id });
                    }
                }
                WorkerEvent::CircuitSimplexGranted { uuid, .. } => {
                    if let Some(call_id) = self.resolve_call(uuid, "SIMPLEX_GRANTED") {
                        Self::signal_cmce(queue, BrewEvent::SimplexGranted { call_id });
                    }
                }
                WorkerEvent::CircuitSimplexIdle { uuid, .. } => {
                    if let Some(call_id) = self.resolve_call(uuid, "SIMPLEX_IDLE") {
                        Self::signal_cmce(queue, BrewEvent::SimplexIdle { call_id });
                    }
                }
                WorkerEvent::CircuitRelease { uuid, cause } => {
                    self.dl_jitter.remove(&uuid);
                    match self.callid_by_uuid(uuid) {
                        Some(call_id) => Self::signal_cmce(queue, BrewEvent::Release { call_id, cause }),
                        None => self.cancel_pending_call(uuid, "CALL_RELEASE"),
                    }
                }
                WorkerEvent::VoiceFrame { uuid, data, .. } => {
                    self.handle_voice_frame(uuid, data);
                }
                WorkerEvent::SdsTransfer {
                    uuid,
                    source,
                    destination,
                    data,
                    length_bits,
                } => {
                    self.handle_sds_transfer(queue, uuid, source, destination, data, length_bits);
                }
                WorkerEvent::SdsReport { uuid, status } => {
                    tracing::debug!("BrewEntity: SDS report uuid={} status={}", uuid, status);
                }
                WorkerEvent::SubscriberEvent { msg_type, issi, groups } => {
                    tracing::debug!("BrewEntity: subscriber event type={} issi={} groups={:?}", msg_type, issi, groups);
                }
                WorkerEvent::ServerError { error_type, data } => {
                    tracing::error!("BrewEntity: server error type={} data={} bytes", error_type, data.len());
                }
            }
        }
    }

    fn handle_subscriber_update(&mut self, update: MmSubscriberUpdate) {
        let issi = update.issi;
        let groups = update.groups;
        let routable = super::is_brew_issi_routable(&self.config, issi);

        match update.action {
            BrewSubscriberAction::Register => {
                self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                if routable {
                    tracing::info!("BrewEntity: subscriber register issi={} → REGISTER", issi);
                    let _ = self.command_sender.send(BrewCommand::RegisterSubscriber { issi });
                } else {
                    tracing::debug!("BrewEntity: subscriber register issi={} (filtered, not sent to Brew)", issi);
                }
            }
            BrewSubscriberAction::Deregister => {
                let existing_groups: Vec<u32> = self
                    .subscriber_groups
                    .remove(&issi)
                    .map(|g| g.into_iter().collect())
                    .unwrap_or_default();
                if routable {
                    tracing::info!("BrewEntity: subscriber deregister issi={} → DEAFFILIATE + DEREGISTER", issi);
                    if !existing_groups.is_empty() {
                        let _ = self.command_sender.send(BrewCommand::DeaffiliateGroups {
                            issi,
                            groups: existing_groups,
                        });
                    }
                    let _ = self.command_sender.send(BrewCommand::DeregisterSubscriber { issi });
                } else {
                    tracing::debug!("BrewEntity: subscriber deregister issi={} (filtered, not sent to Brew)", issi);
                }
            }
            BrewSubscriberAction::Affiliate => {
                let entry = self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                let mut new_groups = Vec::new();
                for gssi in groups {
                    if entry.insert(gssi) {
                        new_groups.push(gssi);
                    }
                }
                if !new_groups.is_empty() && routable {
                    tracing::info!("BrewEntity: affiliate issi={} → AFFILIATE groups={:?}", issi, new_groups);
                    let _ = self.command_sender.send(BrewCommand::AffiliateGroups { issi, groups: new_groups });
                } else if !routable {
                    tracing::debug!(
                        "BrewEntity: affiliate issi={} groups={:?} (filtered, not sent to Brew)",
                        issi,
                        new_groups
                    );
                }
            }
            BrewSubscriberAction::Deaffiliate => {
                let mut removed_groups = Vec::new();
                if let Some(entry) = self.subscriber_groups.get_mut(&issi) {
                    for gssi in groups {
                        if entry.remove(&gssi) {
                            removed_groups.push(gssi);
                        }
                    }
                }
                if !removed_groups.is_empty() && routable {
                    tracing::info!("BrewEntity: deaffiliate issi={} → DEAFFILIATE groups={:?}", issi, removed_groups);
                    let _ = self.command_sender.send(BrewCommand::DeaffiliateGroups {
                        issi,
                        groups: removed_groups,
                    });
                } else if !routable {
                    tracing::debug!(
                        "BrewEntity: deaffiliate issi={} groups={:?} (filtered, not sent to Brew)",
                        issi,
                        removed_groups
                    );
                }
            }
        }
    }

    fn resync_subscribers(&self) {
        for (issi, groups) in &self.subscriber_groups {
            if !super::is_brew_issi_routable(&self.config, *issi) {
                tracing::debug!("BrewEntity: resync skipping issi={} (filtered)", issi);
                continue;
            }
            let _ = self.command_sender.send(BrewCommand::RegisterSubscriber { issi: *issi });
            if groups.is_empty() {
                tracing::info!("BrewEntity: resync issi={} — registered, no group affiliations", issi);
            } else {
                let gssi_list: Vec<u32> = groups.iter().copied().collect();
                tracing::info!(
                    "BrewEntity: resync issi={} — registered, affiliating {} groups: {:?}",
                    issi,
                    gssi_list.len(),
                    gssi_list
                );
                let _ = self.command_sender.send(BrewCommand::AffiliateGroups {
                    issi: *issi,
                    groups: gssi_list,
                });
            }
        }
    }

    /// Handle a transmission started by the backend: a new group call, or a new speaker on an
    /// existing one. CMCE owns circuit allocation and reuse, so only the parties are handed over.
    fn handle_group_call_start(&mut self, queue: &mut MessageQueue, uuid: Uuid, source_issi: u32, dest_gssi: u32, priority: u8) {
        // Repeated GROUP_TX for a session that already holds the floor: nothing changed.
        if let Some(circuit) = self.circuit_by_uuid(uuid)
            && circuit.is_tx()
            && circuit.floor == Some(source_issi)
        {
            tracing::trace!("BrewEntity: repeated GROUP_TX on uuid={} speaker={}", uuid, source_issi);
            return;
        }

        tracing::info!(
            "BrewEntity: network transmission uuid={} src={} gssi={}",
            uuid,
            source_issi,
            dest_gssi
        );

        self.put_network_request(
            uuid,
            NetworkCallRequest {
                source: source_issi,
                destination: dest_gssi,
                priority,
                ..Default::default()
            },
        );
        Self::signal_cmce(queue, BrewEvent::TxStart { brew_uuid: uuid });

        // Buffer downlink voice from here on: the backend streams while CMCE still sets up.
        self.jitter_buffer(uuid);
    }

    /// Handle GROUP_IDLE: the backend transmission is over. CMCE decides whether the call
    /// enters hangtime or is released.
    fn handle_group_call_end(&mut self, queue: &mut MessageQueue, uuid: Uuid, cause: u8) {
        self.dl_jitter.remove(&uuid);

        let Some(call_id) = self.callid_by_uuid(uuid) else {
            // The transmission ended before CMCE turned it into a call.
            self.cancel_pending_call(uuid, "GROUP_IDLE");
            return;
        };

        tracing::info!(
            "BrewEntity: network transmission ended uuid={} call_id={} cause={}",
            uuid,
            call_id,
            cause
        );
        Self::signal_cmce(queue, BrewEvent::TxEnd { call_id });
    }

    /// Handle a voice frame from Brew — buffer it for downlink playout
    fn handle_voice_frame(&mut self, uuid: Uuid, data: Vec<u8>) {
        if !self.is_known_session(uuid) {
            // Might arrive before the call is set up or after it was torn down.
            tracing::trace!("BrewEntity: voice frame for unknown uuid={} ({} bytes)", uuid, data.len());
            return;
        }

        // STE format: byte 0 = header (control bits), bytes 1-35 = 274 ACELP bits for TCH/S.
        // Strip the STE header and buffer only the ACELP payload.
        if data.len() < 36 {
            tracing::warn!("BrewEntity: voice frame too short ({} bytes, expected 36 STE bytes)", data.len());
            return;
        }
        let acelp_data = data[1..].to_vec(); // 35 bytes = 280 bits, of which 274 are ACELP

        self.jitter_buffer(uuid).push(acelp_data);
    }

    /// Feed one buffered frame to every traffic slot whose call is fed from the backend.
    fn drain_jitter_playout(&mut self, queue: &mut MessageQueue) {
        if self.dltime.f == 18 || self.dl_jitter.is_empty() {
            return;
        }
        let ts = self.dltime.t;

        // The circuit view decides which session plays out where, and which are gone for good.
        let (playing, stale): (Vec<Uuid>, Vec<Uuid>) = self.state.with_circuits(|c| {
            let mut playing = Vec::new();
            let mut stale = Vec::new();
            for uuid in self.dl_jitter.keys().copied() {
                match c.get_circuit_by_uuid(uuid) {
                    Some(circuit) if circuit.dl_ts() == ts => playing.push(uuid),
                    Some(_) => {}
                    // No call yet: a setup is still in flight, keep buffering.
                    None if c.has_network_request(uuid) => {}
                    None => stale.push(uuid),
                }
            }
            (playing, stale)
        });

        for uuid in stale {
            tracing::debug!("BrewEntity: dropping playout buffer of gone session uuid={}", uuid);
            self.dl_jitter.remove(&uuid);
        }

        for uuid in playing {
            let Some(jitter) = self.dl_jitter.get_mut(&uuid) else {
                continue;
            };
            jitter.maybe_warn_unhealthy(uuid);
            let target_frames = jitter.target_frames();
            let Some(frame) = jitter.pop_ready() else {
                continue;
            };

            tracing::trace!(
                "BrewEntity: playout uuid={} ts={} rx_seq={} age_ms={} target_frames={}",
                uuid,
                ts,
                frame.rx_seq,
                frame.rx_at.elapsed().as_millis(),
                target_frames
            );
            queue.push_back(SapMsg {
                sap: Sap::TmdSap,
                src: TetraEntity::Brew,
                dest: TetraEntity::Umac,
                msg: SapMsgInner::TmdCircuitDataReq(TmdCircuitDataReq {
                    ts,
                    data: frame.acelp_data,
                }),
            });
        }
    }

    /// Tear down every call that rides the backhaul (on disconnect)
    fn release_all_calls(&mut self, queue: &mut MessageQueue) {
        let sessions: Vec<(u16, Uuid, bool, bool)> = self.state.with_circuits(|c| {
            c.get_circuits()
                .values()
                .filter_map(|circuit| {
                    circuit
                        .brew_uuid()
                        .map(|uuid| (circuit.call_id, uuid, circuit.is_group_call(), circuit.ul1_source.is_local()))
                })
                .collect()
        });

        for (call_id, uuid, is_group, local_speaker) in sessions {
            if !is_group {
                // An individual/PBX call has no leg left without the backhaul.
                Self::signal_cmce(queue, BrewEvent::Release { call_id, cause: 0 });
            } else if local_speaker {
                // The group call is carried on air by a local speaker: only forwarding stops.
                self.detach_session(uuid);
            } else {
                // The backend fed this group call, nothing more will arrive.
                Self::signal_cmce(queue, BrewEvent::TxEnd { call_id });
            }
        }

        self.dl_jitter.clear();
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
        // Feed one buffered frame at each traffic playout opportunity.
        self.drain_jitter_playout(queue);
    }

    fn rx_prim(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        match message.msg {
            // UL voice from UMAC — forward to the backend if this timeslot is being forwarded
            SapMsgInner::TmdCircuitDataInd(prim) => {
                self.handle_ul_voice(prim.ts, prim.data);
            }
            // Call lifecycle notifications from CMCE
            SapMsgInner::CmceCallEvent(event) => {
                self.rx_cmce_event(event);
            }
            SapMsgInner::MmSubscriberUpdate(update) => {
                self.handle_subscriber_update(update);
            }
            SapMsgInner::CmceSdsData(sds) => {
                self.handle_sds_send(sds);
            }
            _ => {
                tracing::debug!("BrewEntity: unexpected rx_prim from {:?} on {:?}", message.src, message.sap);
            }
        }
    }
}

// ─── Call events from CMCE ────────────────────────────────────────

impl BrewEntity {
    fn rx_cmce_event(&mut self, event: CmceEvent) {
        match event {
            // A call the backend offered that never became one: it is named by its session.
            CmceEvent::SetupReject { brew_uuid, cause } => {
                self.dl_jitter.remove(&brew_uuid);
                let _ = self.command_sender.send(BrewCommand::SendSetupReject { uuid: brew_uuid, cause });
                return;
            }
            // A local party taking the floor is what opens a session in the first place.
            CmceEvent::TxStart { call_id } => {
                self.handle_local_tx_start(call_id);
                return;
            }
            _ => {}
        }

        let call_id = event.get_call_id().expect("only the reject names no call");
        let Some(uuid) = self.session_of(call_id) else {
            tracing::warn!("BrewEntity: {:?} for a call without a Brew session", event);
            return;
        };

        match event {
            CmceEvent::TxStart { .. } | CmceEvent::SetupReject { .. } => unreachable!("handled above"),
            CmceEvent::TxEnd { .. } => {
                self.handle_local_tx_end(uuid);
            }
            CmceEvent::SetupRequest { .. } | CmceEvent::ConnectRequest { .. } => {
                let Some(request) = self.take_network_request(uuid) else {
                    tracing::warn!("BrewEntity: {:?} without call parameters", event);
                    return;
                };
                let call = Self::network_request_to_brew(&request);
                let _ = if matches!(event, CmceEvent::SetupRequest { .. }) {
                    self.command_sender.send(BrewCommand::SendSetupRequest { uuid, call })
                } else {
                    self.command_sender.send(BrewCommand::SendConnectRequest { uuid, call })
                };
            }
            CmceEvent::SetupAccept { .. } => {
                let _ = self.command_sender.send(BrewCommand::SendSetupAccept { uuid });
            }
            CmceEvent::Alert { .. } => {
                let _ = self.command_sender.send(BrewCommand::SendCallAlert { uuid });
            }
            CmceEvent::ConnectConfirm { .. } => {
                let _ = self.command_sender.send(BrewCommand::SendConnectConfirm {
                    uuid,
                    grant: TransmissionGrant::Granted.into_raw() as u8,
                    // ETSI Table 14.81: 0 = allowed to request transmission.
                    permission: 0,
                });
            }
            CmceEvent::Release { cause, .. } => {
                self.dl_jitter.remove(&uuid);
                let _ = self.command_sender.send(BrewCommand::SendCallRelease { uuid, cause });
            }
        }
    }
}

// ─── UL call forwarding to the backend ────────────────────────────

impl BrewEntity {
    /// Handle notification that a local party started transmitting. If the party is routable,
    /// open (or reuse) a Brew session on the call and forward the transmission upstream.
    fn handle_local_tx_start(&mut self, call_id: u16) {
        if !self.state_is_connected() {
            tracing::trace!("BrewEntity: not connected, ignoring local transmission start");
            return;
        }
        let Some(circuit) = self.circuit_by_callid(call_id) else {
            tracing::warn!("BrewEntity: transmission start for unknown call_id={}", call_id);
            return;
        };

        let source_issi = circuit.floor.unwrap_or(circuit.caller);
        let dest_gssi = circuit.callee;
        if !super::is_brew_issi_routable(&self.config, source_issi) {
            tracing::debug!(
                "BrewEntity: suppressing GROUP_TX for source_issi={} (filtered, not sent to Brew)",
                source_issi
            );
            return;
        }

        // Keep the session while we are the ones feeding it; a session fed by the backend
        // belongs to the previous speaker, so a talker change starts a fresh one.
        let uuid = match circuit.brew_uuid() {
            Some(uuid) if circuit.ul1_source.is_local() => uuid,
            _ => {
                let uuid = Uuid::new_v4();
                let ts = circuit.dl_ts();
                self.state.with_circuits(|c| {
                    c.update_circuit_with(call_id, |circ| {
                        circ.dl1_source = CircuitStreamDest::LocalAndRemote(Some(ts), Some(uuid))
                    })
                });
                uuid
            }
        };

        tracing::info!(
            "BrewEntity: forwarding local transmission call_id={} src={} dest={} uuid={}",
            call_id,
            source_issi,
            dest_gssi,
            uuid
        );
        let _ = self.command_sender.send(BrewCommand::SendGroupTx {
            uuid,
            source_issi,
            dest_gssi,
            priority: 0,
            service: 0, // TETRA encoded speech
        });
    }

    /// Handle notification that the transmission we forward has stopped. The call itself may
    /// live on (hangtime, new speaker), so only the upstream session is closed.
    fn handle_local_tx_end(&mut self, uuid: Uuid) {
        tracing::info!("BrewEntity: local transmission stopped, sending GROUP_IDLE uuid={}", uuid);
        let _ = self.command_sender.send(BrewCommand::SendGroupIdle {
            uuid,
            cause: 0, // Normal release
        });
        self.dl_jitter.remove(&uuid);
        self.detach_session(uuid);
    }

    /// Handle UL voice data from UMAC. If the call on this timeslot has a Brew session,
    /// convert to STE format and send.
    fn handle_ul_voice(&mut self, ts: u8, acelp_bits: Vec<u8>) {
        let Some(circuit) = self.circuit_by_ul_ts(ts) else {
            return; // No call transmitting on this timeslot
        };
        let Some(uuid) = circuit.brew_uuid() else {
            return; // Not forwarded to the backend
        };

        // Convert ACELP bits to STE format.
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
            uuid,
            length_bits: (ste_data.len() * 8) as u16,
            data: ste_data,
        });
    }
}

// ─── SDS handling ─────────────────────────────────────────────────

impl BrewEntity {
    /// Handle incoming SDS transfer from Brew (network → local MS)
    fn handle_sds_transfer(
        &mut self,
        queue: &mut MessageQueue,
        uuid: Uuid,
        source: u32,
        destination: u32,
        data: Vec<u8>,
        length_bits: u16,
    ) {
        tracing::info!(
            "BrewEntity: SDS transfer uuid={} src={} dst={} {} bytes",
            uuid,
            source,
            destination,
            data.len()
        );

        // Only forward and acknowledge if destination ISSI is locally registered
        let is_local = self.state.with_subscribers(|s| s.is_registered(destination));
        if !is_local {
            tracing::warn!(
                "BrewEntity: SDS dest ISSI {} not registered, dropping (no report sent) uuid={}",
                destination,
                uuid
            );
            return;
        }

        // Brew protocol always delivers SDS as variable-length (Type 4). This means the
        // downlink D-SDS-DATA will use SDTI=3, even if the original uplink was a 16-bit
        // pre-coded status (SDTI=0 / Type 1). This is a Brew protocol constraint.
        let user_defined_data = SdsUserData::Type4(length_bits, data);

        // Forward to CMCE SDS subentity for downlink delivery
        // Set dltime to next ts1 to ensure it gets sent on MCCH
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Cmce,
            msg: SapMsgInner::CmceSdsData(CmceSdsData {
                source_issi: source,
                dest_issi: destination,
                user_defined_data,
            }),
        });

        // Send SDS_REPORT (status=0) back to Brew to release session resources.
        // Without this, sessions are killed by timeout instead of being released cleanly.
        // TODO: should be sent after the radio ACKs on the air interface (LLC BL-ACK),
        // currently sent immediately after queuing for delivery.
        let _ = self.command_sender.send(BrewCommand::SendSdsReport { uuid, status: 0 });
        tracing::info!("BrewEntity: SDS_REPORT uuid={} status=0 -> Brew", uuid);
    }

    /// Handle outgoing SDS from CMCE → Brew (local MS → network)
    fn handle_sds_send(&self, sds: CmceSdsData) {
        if !self.state_is_connected() {
            tracing::warn!(
                "BrewEntity: not connected, dropping outgoing SDS {} -> {}",
                sds.source_issi,
                sds.dest_issi
            );
            return;
        }

        let uuid = Uuid::new_v4();
        tracing::info!(
            "BrewEntity: sending SDS uuid={} src={} dst={} type={} {} bits",
            uuid,
            sds.source_issi,
            sds.dest_issi,
            sds.user_defined_data.type_identifier(),
            sds.user_defined_data.length_bits()
        );

        let _ = self.command_sender.send(BrewCommand::SendSds {
            uuid,
            source: sds.source_issi,
            destination: sds.dest_issi,
            data: sds.user_defined_data.to_arr(),
            length_bits: sds.user_defined_data.length_bits(),
        });
    }
}

impl Drop for BrewEntity {
    fn drop(&mut self) {
        tracing::debug!("BrewEntity: shutting down, sending graceful disconnect");
        let _ = self.command_sender.send(BrewCommand::Disconnect);

        // Give the worker thread time to send DEAFFILIATE + DEREGISTER and close
        if let Some(handle) = self.worker_handle.take() {
            let timeout = std::time::Duration::from_secs(3);
            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    tracing::debug!("BrewEntity: worker thread joined cleanly");
                    break;
                }
                if start.elapsed() >= timeout {
                    tracing::warn!("BrewEntity: worker thread did not finish in time, abandoning");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

// Circuit (individual/PBX/phone) call helpers

impl BrewEntity {
    fn brew_to_network_request(c: &BrewCircularCall) -> NetworkCallRequest {
        NetworkCallRequest {
            source: c.source,
            destination: c.destination,
            number: c.number.clone(),
            priority: c.priority,
            service: c.service,
            mode: c.mode,
            duplex: c.duplex,
            method: c.method,
            communication: c.communication,
            grant: c.grant,
            permission: c.permission,
            timeout: c.timeout,
            ownership: c.ownership,
            queued: c.queued,
        }
    }

    fn network_request_to_brew(c: &NetworkCallRequest) -> BrewCircularCall {
        BrewCircularCall {
            source: c.source,
            destination: c.destination,
            number: c.number.clone(),
            priority: c.priority,
            service: c.service,
            mode: c.mode,
            duplex: c.duplex,
            method: c.method,
            communication: c.communication,
            grant: c.grant,
            permission: c.permission,
            timeout: c.timeout,
            ownership: c.ownership,
            queued: c.queued,
        }
    }
}
