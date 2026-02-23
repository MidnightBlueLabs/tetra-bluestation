//! Brew protocol entity bridging TetraPack WebSocket to UMAC/MLE with hangtime-based circuit reuse

use std::collections::{HashMap, HashSet, VecDeque};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use uuid::Uuid;

use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::SharedConfig;
use tetra_core::{Sap, TdmaTime, tetra_entities::TetraEntity};
use tetra_saps::control::brew::{BrewSubscriberAction, BrewSubscriberUpdate};
use tetra_saps::{SapMsg, SapMsgInner, control::call_control::CallControl, tmd::TmdCircuitDataReq};

use super::worker::{BrewCommand, BrewConfig, BrewEvent, BrewWorker};

/// Hangtime before releasing group call circuit to allow reuse without re-signaling.
const GROUP_CALL_HANGTIME: Duration = Duration::from_secs(1);
/// Minimum playout buffer depth in frames.
const BREW_JITTER_MIN_FRAMES: usize = 2;
/// Default playout buffer depth in frames.
const BREW_JITTER_BASE_FRAMES: usize = 4;
/// Maximum adaptive playout target depth in frames.
const BREW_JITTER_TARGET_MAX_FRAMES: usize = 12;
/// Maximum queued frames kept per call before oldest frames are dropped.
const BREW_JITTER_MAX_FRAMES: usize = 24;
/// Expected receive interval for one TCH/S frame in microseconds (~56.67 ms).
const BREW_EXPECTED_FRAME_INTERVAL_US: f64 = 56_667.0;
/// Warn threshold for excessive adaptive playout depth.
const BREW_JITTER_WARN_TARGET_FRAMES: usize = 8;
/// Rate-limit warning logs per call.
const BREW_JITTER_WARN_INTERVAL: Duration = Duration::from_secs(5);

// ─── Active call tracking ─────────────────────────────────────────

/// Tracks the state of a single active Brew group call (currently transmitting)
#[derive(Debug)]
struct ActiveCall {
    /// Brew session UUID
    uuid: Uuid,
    /// TETRA call identifier (14-bit) - None until NetworkCallReady received
    call_id: Option<u16>,
    /// Allocated timeslot (2-4) - None until NetworkCallReady received
    ts: Option<u8>,
    /// Usage number for the channel allocation - None until NetworkCallReady received
    usage: Option<u8>,
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
    /// Brew session UUID
    uuid: Uuid,
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

#[derive(Debug)]
struct JitterFrame {
    rx_seq: u64,
    rx_at: Instant,
    acelp_data: Vec<u8>,
}

#[derive(Debug, Default)]
struct VoiceJitterBuffer {
    frames: VecDeque<JitterFrame>,
    next_rx_seq: u64,
    started: bool,
    target_frames: usize,
    prev_rx_at: Option<Instant>,
    jitter_us_ewma: f64,
    underrun_boost: usize,
    stable_pops: u32,
    dropped_overflow: u64,
    underruns: u64,
    last_warn_at: Option<Instant>,
    initial_latency_frames: usize,
}

impl VoiceJitterBuffer {
    fn with_initial_latency(initial_latency_frames: usize) -> Self {
        let initial = initial_latency_frames.min(BREW_JITTER_TARGET_MAX_FRAMES - BREW_JITTER_MIN_FRAMES);
        Self {
            target_frames: BREW_JITTER_BASE_FRAMES + initial,
            initial_latency_frames: initial,
            ..Default::default()
        }
    }

    fn push(&mut self, acelp_data: Vec<u8>) {
        if self.target_frames == 0 {
            self.target_frames = BREW_JITTER_BASE_FRAMES + self.initial_latency_frames;
        }
        let now = Instant::now();
        if let Some(prev) = self.prev_rx_at {
            let delta_us = now.duration_since(prev).as_micros() as f64;
            let deviation_us = (delta_us - BREW_EXPECTED_FRAME_INTERVAL_US).abs();
            self.jitter_us_ewma += (deviation_us - self.jitter_us_ewma) / 16.0;
        }
        self.prev_rx_at = Some(now);

        let frame = JitterFrame {
            rx_seq: self.next_rx_seq,
            rx_at: now,
            acelp_data,
        };
        self.next_rx_seq = self.next_rx_seq.wrapping_add(1);
        self.frames.push_back(frame);
        while self.frames.len() > BREW_JITTER_MAX_FRAMES {
            self.frames.pop_front();
            self.dropped_overflow += 1;
        }
        self.recompute_target();
    }

    fn pop_ready(&mut self) -> Option<JitterFrame> {
        if self.target_frames == 0 {
            self.target_frames = BREW_JITTER_BASE_FRAMES + self.initial_latency_frames;
        }

        if !self.started {
            if self.frames.len() < self.target_frames {
                return None;
            }
            self.started = true;
        }

        match self.frames.pop_front() {
            Some(frame) => {
                if self.frames.len() >= self.target_frames {
                    self.stable_pops = self.stable_pops.saturating_add(1);
                    if self.stable_pops >= 80 {
                        self.stable_pops = 0;
                        if self.underrun_boost > 0 {
                            self.underrun_boost -= 1;
                            self.recompute_target();
                        }
                    }
                } else {
                    self.stable_pops = 0;
                }
                Some(frame)
            }
            None => {
                self.started = false;
                self.underruns += 1;
                self.underrun_boost = (self.underrun_boost + 1).min(4);
                self.stable_pops = 0;
                self.recompute_target();
                None
            }
        }
    }

    fn target_frames(&self) -> usize {
        self.target_frames.max(BREW_JITTER_MIN_FRAMES)
    }

    fn recompute_target(&mut self) {
        let jitter_component = ((self.jitter_us_ewma * 2.0) / BREW_EXPECTED_FRAME_INTERVAL_US).ceil() as usize;
        let target = BREW_JITTER_BASE_FRAMES + self.initial_latency_frames + jitter_component + self.underrun_boost;
        self.target_frames = target.clamp(BREW_JITTER_MIN_FRAMES, BREW_JITTER_TARGET_MAX_FRAMES);
    }

    fn maybe_warn_unhealthy(&mut self, uuid: Uuid) {
        let now = Instant::now();
        if let Some(last_warn) = self.last_warn_at {
            if now.duration_since(last_warn) < BREW_JITTER_WARN_INTERVAL {
                return;
            }
        }

        if self.target_frames() < BREW_JITTER_WARN_TARGET_FRAMES && self.underruns == 0 {
            return;
        }

        self.last_warn_at = Some(now);
        tracing::warn!(
            "BrewEntity: high jitter on uuid={} target_frames={} queue={} underruns={} overflow_drops={} jitter_ms={:.1}",
            uuid,
            self.target_frames(),
            self.frames.len(),
            self.underruns,
            self.dropped_overflow,
            self.jitter_us_ewma / 1000.0
        );
    }
}

// ─── BrewEntity ───────────────────────────────────────────────────

pub struct BrewEntity {
    config: SharedConfig,

    brew_config: BrewConfig,

    dltime: TdmaTime,

    /// Receive events from the worker thread
    event_receiver: Receiver<BrewEvent>,
    /// Send commands to the worker thread
    command_sender: Sender<BrewCommand>,

    /// Active DL calls from Brew keyed by session UUID (currently transmitting)
    active_calls: HashMap<Uuid, ActiveCall>,
    /// Per-call jitter/playout buffer for downlink voice from Brew.
    dl_jitter: HashMap<Uuid, VoiceJitterBuffer>,

    /// DL calls in hangtime keyed by dest_gssi — circuit stays open, waiting for
    /// new speaker or timeout. Only one hanging call per GSSI.
    hanging_calls: HashMap<u32, HangingCall>,

    /// UL calls being forwarded to TetraPack, keyed by timeslot
    ul_forwarded: HashMap<u8, UlForwardedCall>,

    /// Registered subscriber groups (ISSI -> set of GSSIs)
    subscriber_groups: HashMap<u32, HashSet<u32>>,

    /// Whether the worker is connected
    connected: bool,

    /// Worker thread handle for graceful shutdown
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl BrewEntity {
    pub fn new(config: SharedConfig, brew_config: BrewConfig) -> Self {
        // Create channels
        let (event_sender, event_receiver) = unbounded::<BrewEvent>();
        let (command_sender, command_receiver) = unbounded::<BrewCommand>();

        // Spawn worker thread
        let worker_config = brew_config.clone();
        let handle = thread::Builder::new()
            .name("brew-worker".to_string())
            .spawn(move || {
                let mut worker = BrewWorker::new(worker_config, event_sender, command_receiver);
                worker.run();
            })
            .expect("failed to spawn BrewWorker thread");

        {
            let mut state = config.state_write();
            state.network_connected = false;
        }

        Self {
            config,
            brew_config,
            dltime: TdmaTime::default(),
            event_receiver,
            command_sender,
            active_calls: HashMap::new(),
            dl_jitter: HashMap::new(),
            hanging_calls: HashMap::new(),
            ul_forwarded: HashMap::new(),
            subscriber_groups: HashMap::new(),
            // next_call_id: 100, // Start at 100 to avoid collision with CMCE
            // next_usage: 10,    // Start at 10 to avoid collision
            connected: false,
            worker_handle: Some(handle),
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
                    self.set_network_connected(true);
                }
                BrewEvent::Disconnected(reason) => {
                    tracing::warn!("BrewEntity: disconnected: {}", reason);
                    self.set_network_connected(false);
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
                    self.handle_voice_frame(uuid, length_bits, data);
                }
                BrewEvent::SubscriberEvent { msg_type, issi, groups } => {
                    tracing::debug!("BrewEntity: subscriber event type={} issi={} groups={:?}", msg_type, issi, groups);
                }
                BrewEvent::ServerError { error_type, data } => {
                    tracing::error!("BrewEntity: server error type={} data={} bytes", error_type, data.len());
                }
            }
        }
    }

    fn handle_subscriber_update(&mut self, update: BrewSubscriberUpdate) {
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
                let mut removed_groups = Vec::new();
                if let Some(existing) = self.subscriber_groups.remove(&issi) {
                    for gssi in existing {
                        removed_groups.push(gssi);
                    }
                }
                if !removed_groups.is_empty() {
                    tracing::info!(
                        "BrewEntity: subscriber deaffiliate on deregister issi={} groups={:?}",
                        issi,
                        removed_groups
                    );
                    let _ = self.command_sender.send(BrewCommand::DeaffiliateGroups {
                        issi,
                        groups: removed_groups,
                    });
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
                if let Some(entry) = self.subscriber_groups.get_mut(&issi) {
                    for gssi in groups {
                        if entry.remove(&gssi) {
                            removed_groups.push(gssi);
                        }
                    }
                } else {
                    removed_groups = groups;
                }

                if removed_groups.is_empty() {
                    tracing::debug!("BrewEntity: deaffiliate ignored (no matching groups) issi={}", issi);
                } else {
                    tracing::info!("BrewEntity: subscriber deaffiliate issi={} groups={:?}", issi, removed_groups);
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

    fn set_network_connected(&mut self, connected: bool) {
        self.connected = connected;
        let mut state = self.config.state_write();
        if state.network_connected != connected {
            state.network_connected = connected;
            tracing::info!("BrewEntity: backhaul {}", if connected { "CONNECTED" } else { "DISCONNECTED" });
        }
    }

    /// Handle new group call from Brew, reusing hanging call circuits if available.
    fn handle_group_call_start(&mut self, queue: &mut MessageQueue, uuid: Uuid, source_issi: u32, dest_gssi: u32, priority: u8) {
        // Check if this call is already active (speaker change or repeated GROUP_TX)
        if let Some(call) = self.active_calls.get_mut(&uuid) {
            // Only notify CMCE if the speaker actually changed
            if call.source_issi != source_issi {
                tracing::info!(
                    "BrewEntity: GROUP_TX speaker change on uuid={} new_speaker={} (was {})",
                    uuid,
                    source_issi,
                    call.source_issi
                );
                call.source_issi = source_issi;

                // Forward speaker change to CMCE
                queue.push_back(SapMsg {
                    sap: Sap::Control,
                    src: TetraEntity::Brew,
                    dest: TetraEntity::Cmce,
                    dltime: self.dltime,
                    msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallStart {
                        brew_uuid: uuid,
                        source_issi,
                        dest_gssi,
                        priority,
                    }),
                });
            } else {
                // Repeated GROUP_TX with same speaker - this is normal, just log at trace level
                tracing::trace!("BrewEntity: repeated GROUP_TX on uuid={} speaker={}", uuid, source_issi);
            }
            return;
        }

        // Check if there's a hanging call we can reuse
        if let Some(hanging) = self.hanging_calls.remove(&dest_gssi) {
            tracing::info!(
                "BrewEntity: reusing hanging circuit for gssi={} uuid={} (hangtime {:.1}s)",
                dest_gssi,
                uuid,
                hanging.since.elapsed().as_secs_f32()
            );

            // Track the call - resources will be set by NetworkCallReady
            let call = ActiveCall {
                uuid,
                call_id: None, // Set by NetworkCallReady
                ts: None,      // Set by NetworkCallReady
                usage: None,   // Set by NetworkCallReady
                source_issi,
                dest_gssi,
                frame_count: hanging.frame_count,
            };
            self.active_calls.insert(uuid, call);
            self.dl_jitter
                .entry(uuid)
                .or_insert_with(|| VoiceJitterBuffer::with_initial_latency(self.brew_config.jitter_initial_latency_frames as usize));

            // Forward to CMCE (will reuse circuit automatically)
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Brew,
                dest: TetraEntity::Cmce,
                dltime: self.dltime,
                msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallStart {
                    brew_uuid: uuid,
                    source_issi,
                    dest_gssi,
                    priority,
                }),
            });
            return;
        }

        // New call - track it and request CMCE to allocate and set up
        tracing::info!(
            "BrewEntity: requesting new network call uuid={} src={} gssi={}",
            uuid,
            source_issi,
            dest_gssi
        );

        // Track the call - resources will be set by NetworkCallReady
        let call = ActiveCall {
            uuid,
            call_id: None, // Set by NetworkCallReady
            ts: None,      // Set by NetworkCallReady
            usage: None,   // Set by NetworkCallReady
            source_issi,
            dest_gssi,
            frame_count: 0,
        };
        self.active_calls.insert(uuid, call);
        self.dl_jitter
            .entry(uuid)
            .or_insert_with(|| VoiceJitterBuffer::with_initial_latency(self.brew_config.jitter_initial_latency_frames as usize));

        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Cmce,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallStart {
                brew_uuid: uuid,
                source_issi,
                dest_gssi,
                priority,
            }),
        });
    }

    /// Handle GROUP_IDLE by forwarding to CMCE and tracking for hangtime reuse
    fn handle_group_call_end(&mut self, queue: &mut MessageQueue, uuid: Uuid, _cause: u8) {
        let Some(call) = self.active_calls.remove(&uuid) else {
            tracing::debug!("BrewEntity: GROUP_IDLE for unknown uuid={}", uuid);
            return;
        };
        self.dl_jitter.remove(&uuid);

        tracing::info!(
            "BrewEntity: group call ended uuid={} call_id={:?} gssi={} frames={}",
            uuid,
            call.call_id,
            call.dest_gssi,
            call.frame_count
        );

        // Request CMCE to end the call
        queue.push_back(SapMsg {
            sap: Sap::Control,
            src: TetraEntity::Brew,
            dest: TetraEntity::Cmce,
            dltime: self.dltime,
            msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid: uuid }),
        });

        // Track as hanging for potential reuse (only if resources were allocated)
        if let (Some(call_id), Some(ts), Some(usage)) = (call.call_id, call.ts, call.usage) {
            self.hanging_calls.insert(
                call.dest_gssi,
                HangingCall {
                    uuid,
                    call_id,
                    ts,
                    usage,
                    source_issi: call.source_issi,
                    dest_gssi: call.dest_gssi,
                    frame_count: call.frame_count,
                    since: Instant::now(),
                },
            );
        }
    }

    /// Clean up expired hanging call tracking hints (CMCE already released circuits)
    fn expire_hanging_calls(&mut self, _queue: &mut MessageQueue) {
        let expired: Vec<u32> = self
            .hanging_calls
            .iter()
            .filter(|(_, h)| h.since.elapsed() >= GROUP_CALL_HANGTIME)
            .map(|(gssi, _)| *gssi)
            .collect();

        for gssi in expired {
            if let Some(hanging) = self.hanging_calls.remove(&gssi) {
                tracing::debug!("BrewEntity: hanging call expired gssi={} uuid={} (no reuse)", gssi, hanging.uuid);
                // No action needed - CMCE already released the circuit
            }
        }
    }

    /// Handle a voice frame from Brew — inject into the downlink
    fn handle_voice_frame(&mut self, uuid: Uuid, _length_bits: u16, data: Vec<u8>) {
        let Some(call) = self.active_calls.get_mut(&uuid) else {
            // Voice frame for unknown call — might arrive before GROUP_TX or after GROUP_IDLE
            tracing::trace!("BrewEntity: voice frame for unknown uuid={} ({} bytes)", uuid, data.len());
            return;
        };

        call.frame_count += 1;

        // Check if resources have been allocated yet
        let Some(ts) = call.ts else {
            // Audio arrived before NetworkCallReady - drop it
            if call.frame_count == 1 {
                tracing::debug!(
                    "BrewEntity: voice frame arrived before resources allocated, uuid={}, dropping",
                    uuid
                );
            }
            return;
        };

        // Log first voice frame per call
        if call.frame_count == 1 {
            tracing::info!(
                "BrewEntity: voice frame #{} uuid={} len={} bytes ts={}",
                call.frame_count,
                uuid,
                data.len(),
                ts
            );
        }

        // STE format: byte 0 = header (control bits), bytes 1-35 = 274 ACELP bits for TCH/S.
        // Strip the STE header and pass only the ACELP payload.
        if data.len() < 36 {
            tracing::warn!("BrewEntity: voice frame too short ({} bytes, expected 36 STE bytes)", data.len());
            return;
        }
        let acelp_data = data[1..].to_vec(); // 35 bytes = 280 bits, of which 274 are ACELP

        self.dl_jitter
            .entry(uuid)
            .or_insert_with(|| VoiceJitterBuffer::with_initial_latency(self.brew_config.jitter_initial_latency_frames as usize))
            .push(acelp_data);
    }

    fn drain_jitter_playout(&mut self, queue: &mut MessageQueue) {
        if self.dltime.f == 18 {
            return;
        }

        let mut to_send: Vec<(u8, Uuid, usize, JitterFrame)> = Vec::new();

        for (uuid, call) in &self.active_calls {
            let Some(ts) = call.ts else {
                continue;
            };
            if ts != self.dltime.t {
                continue;
            }
            let Some(jitter) = self.dl_jitter.get_mut(uuid) else {
                continue;
            };
            jitter.maybe_warn_unhealthy(*uuid);
            if let Some(frame) = jitter.pop_ready() {
                to_send.push((ts, *uuid, jitter.target_frames(), frame));
            }
        }

        for (ts, uuid, target_frames, frame) in to_send {
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
                dltime: self.dltime,
                msg: SapMsgInner::TmdCircuitDataReq(TmdCircuitDataReq {
                    ts,
                    data: frame.acelp_data,
                }),
            });
        }
    }

    /// Release all active calls (on disconnect)
    fn release_all_calls(&mut self, queue: &mut MessageQueue) {
        // Request CMCE to end all active network calls
        let calls: Vec<(Uuid, ActiveCall)> = self.active_calls.drain().collect();
        for (uuid, _) in calls {
            self.dl_jitter.remove(&uuid);
            queue.push_back(SapMsg {
                sap: Sap::Control,
                src: TetraEntity::Brew,
                dest: TetraEntity::Cmce,
                dltime: self.dltime,
                msg: SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid: uuid }),
            });
        }

        // Clear hanging call tracking
        self.hanging_calls.clear();
        self.dl_jitter.clear();
    }

    /// Handle NetworkCallReady response from CMCE
    fn rx_network_call_ready(&mut self, brew_uuid: Uuid, call_id: u16, ts: u8, usage: u8) {
        tracing::info!(
            "BrewEntity: network call ready uuid={} call_id={} ts={} usage={}",
            brew_uuid,
            call_id,
            ts,
            usage
        );

        // Update active call with CMCE-allocated resources
        if let Some(call) = self.active_calls.get_mut(&brew_uuid) {
            call.call_id = Some(call_id);
            call.ts = Some(ts);
            call.usage = Some(usage);
        } else {
            tracing::warn!("BrewEntity: NetworkCallReady for unknown uuid={}", brew_uuid);
        }
    }

    fn drop_network_call(&mut self, brew_uuid: Uuid) {
        if let Some(call) = self.active_calls.remove(&brew_uuid) {
            tracing::info!(
                "BrewEntity: dropping network call uuid={} gssi={} (CMCE request)",
                brew_uuid,
                call.dest_gssi
            );
            self.dl_jitter.remove(&brew_uuid);
            self.hanging_calls.remove(&call.dest_gssi);
            return;
        }

        let hanging_gssi = self
            .hanging_calls
            .iter()
            .find_map(|(gssi, hanging)| if hanging.uuid == brew_uuid { Some(*gssi) } else { None });
        if let Some(gssi) = hanging_gssi {
            tracing::info!("BrewEntity: dropping hanging call uuid={} gssi={} (CMCE request)", brew_uuid, gssi);
            self.hanging_calls.remove(&gssi);
        } else {
            tracing::debug!("BrewEntity: drop requested for unknown uuid={}", brew_uuid);
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
        // Feed one buffered frame at each traffic playout opportunity.
        self.drain_jitter_playout(queue);
        // Expire hanging calls that have exceeded hangtime
        self.expire_hanging_calls(queue);
    }

    fn rx_prim(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        match message.msg {
            // UL voice from UMAC — forward to TetraPack if this timeslot is being forwarded
            SapMsgInner::TmdCircuitDataInd(prim) => {
                self.handle_ul_voice(prim.ts, prim.data);
            }
            // Floor-control and call lifecycle notifications from CMCE
            SapMsgInner::CmceCallControl(CallControl::FloorGranted {
                call_id,
                source_issi,
                dest_gssi,
                ts,
            }) => {
                self.handle_local_call_start(call_id, source_issi, dest_gssi, ts);
            }
            SapMsgInner::CmceCallControl(CallControl::FloorReleased { call_id, ts }) => {
                self.handle_local_call_tx_stopped(call_id, ts);
            }
            SapMsgInner::CmceCallControl(CallControl::CallEnded { call_id, ts }) => {
                self.handle_local_call_end(call_id, ts);
            }
            SapMsgInner::CmceCallControl(CallControl::NetworkCallEnd { brew_uuid }) => {
                self.drop_network_call(brew_uuid);
            }
            SapMsgInner::CmceCallControl(CallControl::NetworkCallReady {
                brew_uuid,
                call_id,
                ts,
                usage,
            }) => {
                self.rx_network_call_ready(brew_uuid, call_id, ts, usage);
            }
            SapMsgInner::BrewSubscriberUpdate(update) => {
                self.handle_subscriber_update(update);
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
        // TODO: Check if local
        // if dest_gssi == 9 {
        //     tracing::debug!(
        //         "BrewEntity: suppressing local call forwarding for TG 9 (call_id={} src={} ts={})",
        //         call_id,
        //         source_issi,
        //         ts
        //     );
        //     return;
        // }

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
    fn handle_local_call_tx_stopped(&mut self, call_id: u16, ts: u8) {
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
                "BrewEntity: local call transmission stopped, sending GROUP_IDLE to TetraPack: uuid={} frames={}",
                fwd.uuid,
                fwd.frame_count
            );
            let _ = self.command_sender.send(BrewCommand::SendGroupIdle {
                uuid: fwd.uuid,
                cause: 0, // Normal release
            });
        }
    }

    fn handle_local_call_end(&mut self, call_id: u16, ts: u8) {
        // Check if ul_forwarded entry still exists (might have been removed by handle_local_call_tx_stopped)
        if let Some(fwd) = self.ul_forwarded.remove(&ts) {
            if fwd.call_id != call_id {
                tracing::warn!(
                    "BrewEntity: call_id mismatch on ts={}: expected {} got {}",
                    ts,
                    fwd.call_id,
                    call_id
                );
            }
            tracing::debug!(
                "BrewEntity: local call ended (already sent GROUP_IDLE during tx_stopped): uuid={} frames={}",
                fwd.uuid,
                fwd.frame_count
            );
        } else {
            tracing::debug!("BrewEntity: local call ended on ts={} (already cleaned up during tx_stopped)", ts);
        }
    }

    /// Handle UL voice data from UMAC. If the timeslot is being forwarded to TetraPack,
    /// convert to STE format and send.
    fn handle_ul_voice(&mut self, ts: u8, acelp_bits: Vec<u8>) {
        let Some(fwd) = self.ul_forwarded.get_mut(&ts) else {
            return; // Not forwarded to TetraPack
        };

        fwd.frame_count += 1;

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
            uuid: fwd.uuid,
            length_bits: (ste_data.len() * 8) as u16,
            data: ste_data,
        });
    }
}

impl Drop for BrewEntity {
    fn drop(&mut self) {
        tracing::info!("BrewEntity: shutting down, sending graceful disconnect");
        let _ = self.command_sender.send(BrewCommand::Disconnect);

        // Give the worker thread time to send DEAFFILIATE + DEREGISTER and close
        if let Some(handle) = self.worker_handle.take() {
            let timeout = std::time::Duration::from_secs(3);
            let start = std::time::Instant::now();
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    tracing::info!("BrewEntity: worker thread joined cleanly");
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
