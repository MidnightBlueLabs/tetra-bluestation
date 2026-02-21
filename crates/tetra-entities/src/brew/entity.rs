//! Brew protocol entity bridging TetraPack WebSocket to UMAC/MLE with hangtime-based circuit reuse

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use uuid::Uuid;

use tetra_config::SharedConfig;
use tetra_core::{Sap, TdmaTime, tetra_entities::TetraEntity};
use tetra_saps::{SapMsg, SapMsgInner, control::call_control::CallControl, tmd::TmdCircuitDataReq};

use crate::{MessageQueue, TetraEntityTrait};

use super::worker::{BrewCommand, BrewConfig, BrewEvent, BrewWorker};

/// Hangtime before releasing group call circuit to allow reuse without re-signaling.
///
/// Keep this short so group switching / rapid re-PTT doesn't feel laggy.
const GROUP_CALL_HANGTIME: Duration = Duration::from_secs(1);

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

    /// Whether the UL transmission is currently active (PTT pressed)
    tx_active: bool,

    /// When tx_active transitioned to false (used to debounce short PTT re-presses)
    tx_stopped_since: Option<Instant>,
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

    /// DL calls in hangtime keyed by dest_gssi — circuit stays open, waiting for
    /// new speaker or timeout. Only one hanging call per GSSI.
    hanging_calls: HashMap<u32, HangingCall>,

    /// UL calls being forwarded to TetraPack, keyed by timeslot
    ul_forwarded: HashMap<u8, UlForwardedCall>,

    /// Whether the worker is connected
    connected: bool,

    /// Worker thread handle for graceful shutdown
    worker_handle: Option<thread::JoinHandle<()>>,
}

// When a radio releases PTT, we enter hangtime. Users often re-press quickly.
// If we send GROUP_IDLE immediately, the subsequent GROUP_TX restart adds latency.
// Debounce short re-presses by delaying GROUP_IDLE a little.
const UL_PTT_IDLE_DEBOUNCE_MS: u64 = 250;

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

        Self {
            config,
            brew_config,
            dltime: TdmaTime::default(),
            event_receiver,
            command_sender,
            active_calls: HashMap::new(),
            hanging_calls: HashMap::new(),
            ul_forwarded: HashMap::new(),
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
                    tracing::error!("BrewEntity: server error type={} data={} bytes", error_type, data.len());
                }
            }
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
            .filter(|(_, h)| h.since.elapsed() > GROUP_CALL_HANGTIME)
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
    fn handle_voice_frame(&mut self, queue: &mut MessageQueue, uuid: Uuid, _length_bits: u16, data: Vec<u8>) {
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

        // Inject ACELP frame into the downlink via TMD SAP
        let tmd_msg = SapMsg {
            sap: Sap::TmdSap,
            src: TetraEntity::Brew,
            dest: TetraEntity::Umac,
            dltime: self.dltime,
            msg: SapMsgInner::TmdCircuitDataReq(TmdCircuitDataReq { ts, data: acelp_data }),
        };
        queue.push_back(tmd_msg);
    }

    /// Release all active calls (on disconnect)
    fn release_all_calls(&mut self, queue: &mut MessageQueue) {
        // Request CMCE to end all active network calls
        let calls: Vec<(Uuid, ActiveCall)> = self.active_calls.drain().collect();
        for (uuid, _) in calls {
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
        // Debounce short PTT re-presses on UL by delaying GROUP_IDLE
        self.expire_ul_ptt_idle_debounce();
        // Expire hanging calls that have exceeded hangtime
        self.expire_hanging_calls(queue);
    }

    fn rx_prim(&mut self, _queue: &mut MessageQueue, message: SapMsg) {
        match message.msg {
            // UL voice from UMAC — forward to TetraPack if this timeslot is being forwarded
            SapMsgInner::TmdCircuitDataInd(prim) => {
                self.handle_ul_voice(prim.ts, prim.data);
            }
            // Floor-control notifications from CMCE (used for UL forwarding)
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
            SapMsgInner::CmceCallControl(CallControl::NetworkCallReady {
                brew_uuid,
                call_id,
                ts,
                usage,
            }) => {
                self.rx_network_call_ready(brew_uuid, call_id, ts, usage);
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

        // Check if this group is subscribed in Brew config
        // let groups = &self.config.config().brew.groups;
        let groups = &self.brew_config.groups;
        if !groups.contains(&dest_gssi) {
            tracing::debug!(
                "BrewEntity: local call on GSSI {} not subscribed (subscribed: {:?}), not forwarding",
                dest_gssi,
                groups
            );
            return;
        }

        // If we already have a forwarded UL call on this timeslot, it might be a quick re-PTT
        // during hangtime. In that case, avoid tearing down / restarting the Brew session.
        if let Some(existing) = self.ul_forwarded.get_mut(&ts) {
            // If it's the same group, treat as resume (speaker may change)
            if existing.dest_gssi == dest_gssi {
                tracing::info!(
                    "BrewEntity: resuming local UL call during hangtime: ts={} call_id={}→{} src={}→{} gssi={} uuid={}",
                    ts,
                    existing.call_id,
                    call_id,
                    existing.source_issi,
                    source_issi,
                    dest_gssi,
                    existing.uuid
                );
                existing.call_id = call_id;
                existing.source_issi = source_issi;
                existing.tx_active = true;
                existing.tx_stopped_since = None;
                return;
            }

            // Different group on same ts: close old session first
            tracing::warn!(
                "BrewEntity: replacing UL forwarded call on ts={} old_gssi={} new_gssi={}, sending GROUP_IDLE for old uuid={}",
                ts,
                existing.dest_gssi,
                dest_gssi,
                existing.uuid
            );
            let _ = self.command_sender.send(BrewCommand::SendGroupIdle { uuid: existing.uuid, cause: 0 });
        }

        // New forwarded UL call
        let uuid = Uuid::new_v4();
        tracing::info!(
            "BrewEntity: forwarding local call to TetraPack: call_id={} src={} gssi={} ts={} uuid={}",
            call_id,
            source_issi,
            dest_gssi,
            ts,
            uuid
        );

        let _ = self.command_sender.send(BrewCommand::SendGroupTx {
            uuid,
            source_issi,
            dest_gssi,
            priority: 0,
            service: 0, // TETRA encoded speech
        });

        self.ul_forwarded.insert(
            ts,
            UlForwardedCall {
                uuid,
                call_id,
                source_issi,
                dest_gssi,
                frame_count: 0,
                tx_active: true,
                tx_stopped_since: None,
            },
        );
    }

    /// Handle notification that a local UL call has ended.
    fn handle_local_call_tx_stopped(&mut self, call_id: u16, ts: u8) {
        if let Some(fwd) = self.ul_forwarded.get_mut(&ts) {
            if fwd.call_id != call_id {
                tracing::warn!(
                    "BrewEntity: call_id mismatch on ts={}: expected {} got {}",
                    ts,
                    fwd.call_id,
                    call_id
                );
            }
            fwd.tx_active = false;
            fwd.tx_stopped_since = Some(Instant::now());
            tracing::info!(
                "BrewEntity: UL tx stopped on ts={} (debounce {}ms before GROUP_IDLE): uuid={} frames={}",
                ts,
                UL_PTT_IDLE_DEBOUNCE_MS,
                fwd.uuid,
                fwd.frame_count
            );
        }
    }

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
                "BrewEntity: local UL call ended, sending GROUP_IDLE to TetraPack: uuid={} frames={}",
                fwd.uuid,
                fwd.frame_count
            );
            let _ = self.command_sender.send(BrewCommand::SendGroupIdle { uuid: fwd.uuid, cause: 0 });
        }
    }

    fn expire_ul_ptt_idle_debounce(&mut self) {
        if self.ul_forwarded.is_empty() {
            return;
        }

        let now = Instant::now();
        let mut to_close: Vec<u8> = Vec::new();

        for (&ts, fwd) in self.ul_forwarded.iter() {
            if fwd.tx_active {
                continue;
            }
            let Some(since) = fwd.tx_stopped_since else {
                continue;
            };
            if now.duration_since(since) >= std::time::Duration::from_millis(UL_PTT_IDLE_DEBOUNCE_MS) {
                to_close.push(ts);
            }
        }

        for ts in to_close {
            if let Some(fwd) = self.ul_forwarded.remove(&ts) {
                tracing::info!(
                    "BrewEntity: debounce expired, sending GROUP_IDLE for ts={} uuid={} frames={}",
                    ts,
                    fwd.uuid,
                    fwd.frame_count
                );
                let _ = self.command_sender.send(BrewCommand::SendGroupIdle { uuid: fwd.uuid, cause: 0 });
            }
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
