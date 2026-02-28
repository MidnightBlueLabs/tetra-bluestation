//! Brew protocol entity bridging TetraPack WebSocket to UMAC/MLE with hangtime-based circuit reuse

use std::collections::{HashMap, HashSet, VecDeque};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use uuid::Uuid;

use crate::{MessageQueue, TetraEntityTrait};
use tetra_config::bluestation::{CfgBrew, SharedConfig};
use tetra_core::{Sap, TdmaTime, tetra_entities::TetraEntity, BitBuffer, TetraAddress, address::SsiType};
use tetra_saps::lcmc::LcmcMleUnitdataReq;
use tetra_saps::control::brew::{BrewSubscriberAction, MmSubscriberUpdate};
use tetra_saps::{SapMsg, SapMsgInner, control::call_control::CallControl, tmd::TmdCircuitDataReq};

use super::worker::{BrewCommand, BrewEvent, BrewWorker};

/// Hangtime before releasing group call circuit to allow reuse without re-signaling.
const GROUP_CALL_HANGTIME: Duration = Duration::from_secs(5);
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

// SDS staging timeouts (Brew SHORT_TRANSFER + SDS_TRANSFER may arrive out-of-order)
const SDS_SESSION_TIMEOUT: Duration = Duration::from_secs(2);

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

#[derive(Debug, Clone)]
struct SdsSessionCtx {
    uuid: Uuid,
    source_issi: u32,
    destination: u32,
    number: String,
    since: Instant,
}

#[derive(Debug, Clone)]
struct PendingSdsPayload {
    uuid: Uuid,
    length_bits: u16,
    data: Vec<u8>,
    since: Instant,
}

// ─── SDS PID=0x04 (observed) delivery report routing ─────────────

#[derive(Debug, Clone)]
struct PendingSdsPid04Report {
    origin_ssi: u32,
    dest_ssi: u32,
    mr: u8,
    since: Instant,
}

fn sds_pid04_key(dest_ssi: u32, mr: u8) -> u64 {
    ((dest_ssi as u64) << 8) | (mr as u64)
}

fn is_sds_pid04(payload: &[u8]) -> bool {
    payload.first().copied() == Some(0x04)
}

// Heuristic: in observed MS, ctrl byte 0x08 indicates Delivery Report Request (DRR=1).
fn pid04_drr_requested(payload: &[u8]) -> bool {
    payload.len() >= 2 && (payload[1] & 0x08) != 0
}

fn pid04_msg_type(payload: &[u8]) -> Option<u8> {
    payload.get(1).map(|b| (b & 0xF0) >> 4)
}

fn is_pid04_status_pdu(payload: &[u8]) -> bool {
    payload.len() >= 4
        && payload.first().copied() == Some(0x04)
        && matches!(pid04_msg_type(payload), Some(1) | Some(2))
}

fn parse_pid04_status(payload: &[u8]) -> Option<(u8, u8, u8)> {
    // Returns (msg_type, status, mr)
    if payload.len() < 4 || payload[0] != 0x04 {
        return None;
    }
    let mt = pid04_msg_type(payload)?;
    if mt != 1 && mt != 2 {
        return None;
    }
    Some((mt, payload[2], payload[3]))
}

// Data observed: 04 <ctrl> <mr> ...
fn parse_pid04_mr_from_data(payload: &[u8]) -> Option<u8> {
    if payload.len() >= 3 && payload[0] == 0x04 {
        if is_pid04_status_pdu(payload) {
            return None;
        }
        return Some(payload[2]);
    }
    None
}

fn build_pid04_report(mr: u8, status: u8) -> [u8; 4] {
    [0x04, 0x10, status, mr]
}

fn build_pid04_ack(mr: u8, status: u8) -> [u8; 4] {
    [0x04, 0x20, status, mr]
}

fn fmt_hex4(payload: &[u8]) -> String {
    let mut out = String::new();
    for (i, b) in payload.iter().take(4).enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{:02X}", b));
    }
    out
}

fn pid04_msg_type_name(mt: u8) -> &'static str {
    match mt {
        1 => "REPORT",
        2 => "ACK",
        _ => "UNKNOWN",
    }
}

fn pid04_delivery_status_name(status: u8) -> &'static str {
    match status {
        0x00 => "Receipt acknowledged by destination",
        0x01 => "Receipt report acknowledgement",
        0x02 => "Consumed by destination",
        0x03 => "Consumed report acknowledgement",
        0x04 => "Forwarded to external network",
        0x05 => "Sent to group, acknowledgements prevented",
        0x20 => "Congestion, message stored by SwMI",
        0x21 => "Message stored by SwMI",
        0x22 => "Destination not reachable, message stored by SwMI",
        0x40 => "Network overload",
        0x41 => "Service permanently not available on BS",
        0x42 => "Service temporarily not available on BS",
        0x43 => "Source not authorized for SDS",
        0x44 => "Destination not authorized for SDS",
        0x45 => "Unknown destination/gateway/service centre",
        0x46 => "Unknown forward address",
        0x47 => "Group address with individual service",
        0x48 => "Validity period expired (not received)",
        0x49 => "Validity period expired (not consumed)",
        0x4A => "Delivery failed",
        0x4B => "Destination not registered on system",
        0x4C => "Destination queue full",
        0x4D => "Message too long for destination/gateway",
        0x4E => "Destination does not support SDS-TL",
        0x4F => "Destination host not connected",
        0x50 => "Protocol not supported",
        0x51 => "Data coding scheme not supported",
        0x52 => "Destination memory full, message discarded",
        0x53 => "Destination not accepting SDS messages",
        0x56 => "Destination address administratively prohibited",
        0x57 => "Cannot route to external network",
        0x58 => "Unknown external subscriber number",
        0x59 => "Negative report acknowledgement",
        0x5A => "Destination not reachable, delivery failed",
        0x60 => "Destination memory full",
        0x61 => "Destination memory available",
        0x62 => "Start pending messages",
        0x63 => "No pending messages",
        0x80 => "Stop sending",
        0x81 => "Start sending",
        _ => {
            if status < 0x20 {
                "SDS transfer success (reserved/unknown)"
            } else if status < 0x40 {
                "Temporary error (reserved/unknown)"
            } else if status < 0x60 {
                "Transfer failed (reserved/unknown)"
            } else if status < 0x80 {
                "Flow control (reserved/unknown)"
            } else {
                "End-to-end control / reserved"
            }
        }
    }
}

// ─── Minimal SDS‑TL (ETSI/TCCA practical) helpers ─────────────────

fn sds_tl_msg_type(payload: &[u8]) -> Option<u8> {
    payload.get(1).map(|b| (b & 0xF0) >> 4)
}

fn is_sds_tl_status_pdu(payload: &[u8]) -> bool {
    matches!(sds_tl_msg_type(payload), Some(1) | Some(2)) && payload.len() >= 4
}

fn parse_sds_tl_transfer_pid_mr(payload: &[u8]) -> Option<(u8, u8)> {
    if payload.len() < 3 {
        return None;
    }
    if sds_tl_msg_type(payload) != Some(0) {
        return None;
    }
    let pid = payload[0];
    let mr = payload[2];
    Some((pid, mr))
}

fn build_sds_tl_ack(pid: u8, status: u8, mr: u8) -> [u8; 4] {
    [pid, 0x20, status, mr]
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

    /// Also contained in the SharedConfig, but kept for fast, convenient access
    brew_config: CfgBrew,

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

    // SDS SHORT_TRANSFER context staging
    sds_sessions: HashMap<Uuid, SdsSessionCtx>,
    sds_pending: HashMap<Uuid, Vec<PendingSdsPayload>>,
    next_sds_dl_ts1: Option<TdmaTime>,

    // Pending PID=0x04 report routing: key(dest_ssi,mr)->origin
    pending_sds_pid04_reports: HashMap<u64, PendingSdsPid04Report>,

    /// Whether the worker is connected
    connected: bool,

    /// Worker thread handle for graceful shutdown
    worker_handle: Option<thread::JoinHandle<()>>,
}

impl BrewEntity {
    pub fn new(config: SharedConfig) -> Self {
        // Create channels
        let (event_sender, event_receiver) = unbounded::<BrewEvent>();
        let (command_sender, command_receiver) = unbounded::<BrewCommand>();

        // Spawn worker thread
        let brew_config = config.config().as_ref().brew.clone().unwrap(); // Never fails
        let worker_config = config.clone();
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
            sds_sessions: HashMap::new(),
            sds_pending: HashMap::new(),
            next_sds_dl_ts1: None,
            pending_sds_pid04_reports: HashMap::new(),
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
                BrewEvent::ShortTransfer {
                    uuid,
                    source_issi,
                    destination,
                    number,
                } => {
                    self.handle_short_transfer(queue, uuid, source_issi, destination, number);
                }
                BrewEvent::SdsTransfer { uuid, length_bits, data } => {
                    self.handle_sds_transfer(queue, uuid, length_bits, data);
                }
                BrewEvent::SdsReport { uuid, status } => {
                    tracing::debug!("BrewEntity: SDS_REPORT uuid={} status={}", uuid, status);
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

    fn set_network_connected(&mut self, connected: bool) {
        self.connected = connected;
        let mut state = self.config.state_write();
        if state.network_connected != connected {
            state.network_connected = connected;
            tracing::info!("BrewEntity: backhaul {}", if connected { "CONNECTED" } else { "DISCONNECTED" });
        }
    }

    fn reserve_sds_dl_ts1(&mut self) -> TdmaTime {
        let candidate = self.dltime.forward_to_timeslot(1);
        let next = match self.next_sds_dl_ts1 {
            None => candidate,
            Some(cur) => {
                if cur.to_int() < candidate.to_int() {
                    candidate
                } else {
                    cur
                }
            }
        };
        self.next_sds_dl_ts1 = Some(next.add_timeslots(4));
        next
    }

    fn expire_sds_staging(&mut self) {
        let now = Instant::now();

        // Expire contexts that never received an SDS_TRANSFER.
        let mut expired_ctx = Vec::new();
        for (uuid, ctx) in self.sds_sessions.iter() {
            if now.duration_since(ctx.since) > SDS_SESSION_TIMEOUT {
                expired_ctx.push(*uuid);
            }
        }
        for uuid in expired_ctx {
            let _ = self.sds_sessions.remove(&uuid);
            tracing::warn!("BrewEntity: SHORT_TRANSFER timed out waiting for SDS_TRANSFER uuid={}", uuid);
            let _ = self.command_sender.send(BrewCommand::SendSdsReport { uuid, status: 1 });
        }

        // Expire payloads that never received a SHORT_TRANSFER.
        let mut expired_payload = Vec::new();
        for (uuid, list) in self.sds_pending.iter() {
            let expired = list
                .first()
                .map(|p| now.duration_since(p.since) > SDS_SESSION_TIMEOUT)
                .unwrap_or(false);
            if expired {
                expired_payload.push(*uuid);
            }
        }
        for uuid in expired_payload {
            let _ = self.sds_pending.remove(&uuid);
            tracing::warn!("BrewEntity: SDS_TRANSFER timed out waiting for SHORT_TRANSFER uuid={}", uuid);
            let _ = self.command_sender.send(BrewCommand::SendSdsReport { uuid, status: 1 });
        }

        // Expire pending PID=0x04 delivery report routes (dest,mr)->origin.
        const PID04_REPORT_TIMEOUT: Duration = Duration::from_secs(8);
        let mut expired_keys = Vec::new();
        for (k, v) in self.pending_sds_pid04_reports.iter() {
            if now.duration_since(v.since) > PID04_REPORT_TIMEOUT {
                expired_keys.push(*k);
            }
        }
        for k in expired_keys {
            let _ = self.pending_sds_pid04_reports.remove(&k);
        }
    }

    fn infer_sds_destination_type(&self, ssi: u32) -> SsiType {
        // Heuristic: if any known subscriber is affiliated to this SSI, treat it as GSSI.
        if self.subscriber_groups.values().any(|set| set.contains(&ssi)) {
            SsiType::Gssi
        } else {
            SsiType::Ssi
        }
    }

    fn parse_sds_dst_hint(&self, number: &str) -> Option<SsiType> {
        let n = number.to_ascii_lowercase();
        if n.contains("dst=gssi") || n.contains("dst:gssi") {
            Some(SsiType::Gssi)
        } else if n.contains("dst=issi") || n.contains("dst:issi") {
            Some(SsiType::Ssi)
        } else {
            None
        }
    }

    fn push_cmce_unitdata_req(&mut self, queue: &mut MessageQueue, address: TetraAddress, mut sdu: BitBuffer, force_mcch: bool, repeats: bool) {
        sdu.seek(0);
        queue.push_back(SapMsg {
            sap: Sap::LcmcSap,
            src: TetraEntity::Cmce,
            dest: TetraEntity::Mle,
            dltime: if force_mcch { self.reserve_sds_dl_ts1() } else { self.dltime },
            msg: SapMsgInner::LcmcMleUnitdataReq(LcmcMleUnitdataReq {
                sdu,
                handle: 0,
                endpoint_id: 0,
                link_id: 3,
                layer2service: 0,
                pdu_prio: 2,
                layer2_qos: 0,
                // For MCCH/TS1 (SDS/signalling), stealing must remain disabled.
                stealing_permission: !force_mcch,
                stealing_repeats_flag: repeats,
                main_address: address,
                chan_alloc: None,
                tx_reporter: None,
            }),
        });
    }

    fn send_pid04_status_to_origin(
        &mut self,
        queue: &mut MessageQueue,
        origin_ssi: u32,
        report_sender_ssi: u32,
        mt: u8,
        mr: u8,
        status: u8,
        repeat: bool,
    ) {
        use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;

        let payload: [u8; 4] = match mt {
            2 => build_pid04_ack(mr, status),
            _ => build_pid04_report(mr, status),
        };

        let addr = TetraAddress::new(origin_ssi, SsiType::Ssi);
        let pdu = DSdsData {
            calling_party_type_identifier: 1,
            calling_party_address_ssi: Some(report_sender_ssi as u64),
            calling_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(32),
            user_defined_data_4: Some(payload.to_vec()),
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(96);
        if pdu.to_bitbuf(&mut sdu).is_err() {
            return;
        }

        // Use MCCH/TS1 pacing. Optionally request one scheduler-level repeat for reliability.
        self.push_cmce_unitdata_req(queue, addr, sdu, true, repeat);
    }

    fn send_sds_tl_ack_to_origin(
        &mut self,
        queue: &mut MessageQueue,
        origin_ssi: u32,
        report_sender_ssi: u32,
        pid: u8,
        mr: u8,
        repeat: bool,
    ) {
        use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;

        let payload = build_sds_tl_ack(pid, 0x00, mr);
        let addr = TetraAddress::new(origin_ssi, SsiType::Ssi);

        let pdu = DSdsData {
            calling_party_type_identifier: 1,
            calling_party_address_ssi: Some(report_sender_ssi as u64),
            calling_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(32),
            user_defined_data_4: Some(payload.to_vec()),
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(96);
        if pdu.to_bitbuf(&mut sdu).is_err() {
            return;
        }

        self.push_cmce_unitdata_req(queue, addr, sdu, true, repeat);
    }

    fn handle_short_transfer(&mut self, queue: &mut MessageQueue, uuid: Uuid, source_issi: u32, destination: u32, number: String) {
        let ctx = SdsSessionCtx {
            uuid,
            source_issi,
            destination,
            number,
            since: Instant::now(),
        };

        // Brew SHORT_TRANSFER and SDS_TRANSFER frames may arrive out-of-order.
        if let Some(pendings) = self.sds_pending.remove(&uuid) {
            for p in pendings {
                self.inject_sds_with_ctx(queue, uuid, ctx.clone(), p.length_bits, p.data);
            }
        } else {
            self.sds_sessions.insert(uuid, ctx);
        }
    }

    fn inject_sds_with_ctx(&mut self, queue: &mut MessageQueue, uuid: Uuid, ctx: SdsSessionCtx, length_bits: u16, data: Vec<u8>) {
        use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;

        let dst_type = self
            .parse_sds_dst_hint(&ctx.number)
            .unwrap_or_else(|| self.infer_sds_destination_type(ctx.destination));
        let addr = TetraAddress::new(ctx.destination, dst_type);

        tracing::info!(
            "BrewEntity: SDS DL uuid={} src={} dst={} bits={} number='{}'",
            uuid,
            ctx.source_issi,
            addr,
            length_bits,
            ctx.number
        );

        let pdu = DSdsData {
            calling_party_type_identifier: 1,
            calling_party_address_ssi: Some(ctx.source_issi as u64),
            calling_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(length_bits),
            user_defined_data_4: Some(data.clone()),
            external_subscriber_number: None,
            dm_ms_address: None,
        };

        let mut sdu = BitBuffer::new_autoexpand(64 + (length_bits as usize));
        if let Err(e) = pdu.to_bitbuf(&mut sdu) {
            tracing::error!("BrewEntity: DSdsData encode failed: {:?}", e);
            let _ = self.command_sender.send(BrewCommand::SendSdsReport { uuid, status: 1 });
            return;
        }

        let repeat = is_sds_pid04(&data) && pid04_drr_requested(&data);
        self.push_cmce_unitdata_req(queue, addr, sdu, true, repeat);
        let _ = self.command_sender.send(BrewCommand::SendSdsReport { uuid, status: 0 });
    }

    fn handle_sds_transfer(&mut self, queue: &mut MessageQueue, uuid: Uuid, length_bits: u16, data: Vec<u8>) {
        if let Some(ctx) = self.sds_sessions.remove(&uuid) {
            self.inject_sds_with_ctx(queue, uuid, ctx, length_bits, data);
            return;
        }

        tracing::warn!("BrewEntity: SDS_TRANSFER without prior SHORT_TRANSFER uuid={} (buffering)", uuid);
        self.sds_pending
            .entry(uuid)
            .or_insert_with(Vec::new)
            .push(PendingSdsPayload {
                uuid,
                length_bits,
                data,
                since: Instant::now(),
            });
    }

    fn handle_uplink_sds(&mut self, queue: &mut MessageQueue, ind: tetra_saps::control::sds::BrewSdsRxInd) {
        // PID=0x04 status (REPORT/ACK) must be routed back to origin.
        if let Some((mt, status, mr)) = parse_pid04_status(&ind.payload) {
            let key = sds_pid04_key(ind.source.ssi, mr);
            if let Some(pend) = self.pending_sds_pid04_reports.remove(&key) {
                tracing::info!(
                    "BrewEntity: PID04 {} raw4={} mr=0x{:02x} status=0x{:02x} ({}) from={} -> origin={} (dst was {})",
                    pid04_msg_type_name(mt),
                    fmt_hex4(&ind.payload),
                    mr,
                    status,
                    pid04_delivery_status_name(status),
                    ind.source.ssi,
                    pend.origin_ssi,
                    pend.dest_ssi
                );
                self.send_pid04_status_to_origin(queue, pend.origin_ssi, ind.source.ssi, mt, mr, status, true);
            } else {
                tracing::debug!(
                    "BrewEntity: dropping PID04 {} raw4={} with no pending mapping from={} mr=0x{:02x} status=0x{:02x}",
                    pid04_msg_type_name(mt),
                    fmt_hex4(&ind.payload),
                    ind.source.ssi,
                    mr,
                    status
                );
            }
            return;
        }

        // SDS‑TL status PDUs (ACK/REPORT) are not user messages. Relay over-air for local MS↔MS.
        if is_sds_tl_status_pdu(&ind.payload) {
            let dst_type = match ind.destination.ssi_type {
                SsiType::Gssi => SsiType::Gssi,
                SsiType::Ssi => SsiType::Ssi,
                _ => self.infer_sds_destination_type(ind.destination.ssi),
            };

            let do_local_relay = dst_type == SsiType::Ssi && ind.destination.ssi != ind.source.ssi;
            if do_local_relay {
                use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;
                let addr = TetraAddress::new(ind.destination.ssi, SsiType::Ssi);
                let pdu = DSdsData {
                    calling_party_type_identifier: 1,
                    calling_party_address_ssi: Some(ind.source.ssi as u64),
                    calling_party_extension: None,
                    short_data_type_identifier: 3,
                    user_defined_data_1: None,
                    user_defined_data_2: None,
                    user_defined_data_3: None,
                    length_indicator: Some(ind.bit_length),
                    user_defined_data_4: Some(ind.payload.clone()),
                    external_subscriber_number: None,
                    dm_ms_address: None,
                };

                let mut sdu = BitBuffer::new_autoexpand(64 + (ind.bit_length as usize));
                if let Err(e) = pdu.to_bitbuf(&mut sdu) {
                    tracing::warn!("BrewEntity: SDS‑TL status relay encode failed: {:?}", e);
                } else {
                    tracing::info!(
                        "BrewEntity: local SDS‑TL status relay src={} dst={} bits={} head4={}",
                        ind.source.ssi,
                        ind.destination.ssi,
                        ind.bit_length,
                        fmt_hex4(&ind.payload)
                    );
                    self.push_cmce_unitdata_req(queue, addr, sdu, true, true);
                }
            }
            return;
        }

        // Forward uplink SDS to core via Brew SHORT_TRANSFER + SDS_TRANSFER.
        let uuid = Uuid::new_v4();
        let dst_type = match ind.destination.ssi_type {
            SsiType::Gssi => SsiType::Gssi,
            SsiType::Ssi => SsiType::Ssi,
            _ => self.infer_sds_destination_type(ind.destination.ssi),
        };

        let mut number = match dst_type {
            SsiType::Gssi => "dst=gssi".to_string(),
            _ => "dst=issi".to_string(),
        };

        let do_local_relay = dst_type == SsiType::Ssi
            && ind.destination.ssi != ind.source.ssi
            && self.subscriber_groups.contains_key(&ind.destination.ssi);

        if do_local_relay && is_sds_pid04(&ind.payload) {
            if let Some(mr) = parse_pid04_mr_from_data(&ind.payload) {
                let key = sds_pid04_key(ind.destination.ssi, mr);
                self.pending_sds_pid04_reports.insert(
                    key,
                    PendingSdsPid04Report {
                        origin_ssi: ind.source.ssi,
                        dest_ssi: ind.destination.ssi,
                        mr,
                        since: Instant::now(),
                    },
                );
            }
        }

        if do_local_relay {
            use tetra_pdus::cmce::pdus::d_sds_data::DSdsData;
            let addr = TetraAddress::new(ind.destination.ssi, SsiType::Ssi);
            let pdu = DSdsData {
                calling_party_type_identifier: 1,
                calling_party_address_ssi: Some(ind.source.ssi as u64),
                calling_party_extension: None,
                short_data_type_identifier: 3,
                user_defined_data_1: None,
                user_defined_data_2: None,
                user_defined_data_3: None,
                length_indicator: Some(ind.bit_length),
                user_defined_data_4: Some(ind.payload.clone()),
                external_subscriber_number: None,
                dm_ms_address: None,
            };

            let mut sdu = BitBuffer::new_autoexpand(64 + (ind.bit_length as usize));
            if let Err(e) = pdu.to_bitbuf(&mut sdu) {
                tracing::warn!("BrewEntity: local SDS relay encode failed: {:?}", e);
            } else {
                tracing::info!(
                    "BrewEntity: local SDS relay src={} dst={} bits={}",
                    ind.source.ssi,
                    ind.destination.ssi,
                    ind.bit_length
                );

                // Many terminals require an SDS‑ACK to clear the sender UI.
                if let Some((pid, mr)) = parse_sds_tl_transfer_pid_mr(&ind.payload) {
                    self.send_sds_tl_ack_to_origin(queue, ind.source.ssi, ind.destination.ssi, pid, mr, true);
                }

                self.push_cmce_unitdata_req(queue, addr, sdu, true, false);
                number.push_str(";local=1");
            }
        }

        if !self.connected {
            tracing::debug!("BrewEntity: dropping uplink SDS (backhaul disconnected) src={} dst={}", ind.source, ind.destination);
            return;
        }

        if !super::is_brew_issi_routable(&self.config, ind.source.ssi) {
            tracing::debug!("BrewEntity: uplink SDS src={} filtered; not forwarded to Brew", ind.source);
            return;
        }

        let _ = self.command_sender.send(BrewCommand::SendShortTransfer {
            uuid,
            source_issi: ind.source.ssi,
            destination: ind.destination.ssi,
            number,
        });
        let _ = self.command_sender.send(BrewCommand::SendSdsTransfer {
            uuid,
            length_bits: ind.bit_length,
            data: ind.payload,
        });
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
        self.expire_sds_staging();
        // Feed one buffered frame at each traffic playout opportunity.
        self.drain_jitter_playout(queue);
        // Expire hanging calls that have exceeded hangtime
        self.expire_hanging_calls(queue);
    }

    fn rx_prim(&mut self, queue: &mut MessageQueue, message: SapMsg) {
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
            SapMsgInner::MmSubscriberUpdate(update) => {
                self.handle_subscriber_update(update);
            }
            SapMsgInner::BrewSdsRxInd(ind) => {
                self.handle_uplink_sds(queue, ind);
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
        if !super::is_brew_issi_routable(&self.config, source_issi) {
            tracing::debug!(
                "BrewEntity: suppressing GROUP_TX for source_issi={} (filtered, not sent to Brew)",
                source_issi
            );
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
