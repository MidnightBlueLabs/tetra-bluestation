//! Brew worker thread: transport-agnostic message loop for Brew protocol exchange
//!
//! Generic over any [`NetworkTransport`] implementation (WebSocket, QUIC, TCP, etc.).
//! The transport handles connection lifecycle and heartbeat; this worker handles
//! Brew protocol parsing, command dispatch, and event generation.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender};
use tetra_config::bluestation::CfgBrew;
use tetra_config::bluestation::SharedConfig;
use uuid::Uuid;

use crate::brew;
use crate::network::transports::NetworkTransport;

use super::protocol::*;

// ─── Events passed from worker to entity ─────────────────────────

/// Events the Brew worker sends to the BrewEntity
#[derive(Debug)]
pub enum BrewEvent {
    /// Successfully connected to TetraPack server
    Connected,

    /// Disconnected (with reason)
    Disconnected(String),

    /// Group call started
    GroupCallStart {
        uuid: Uuid,
        source_issi: u32,
        dest_gssi: u32,
        priority: u8,
        service: u16,
    },

    /// Group call ended
    GroupCallEnd { uuid: Uuid, cause: u8 },

    /// Voice frame received (ACELP traffic)
    VoiceFrame { uuid: Uuid, length_bits: u16, data: Vec<u8> },

    /// Subscriber event received
    SubscriberEvent { msg_type: u8, issi: u32, groups: Vec<u32> },

    /// SDS transfer received (SHORT_TRANSFER + SDS_TRANSFER combined)
    SdsTransfer {
        uuid: Uuid,
        source: u32,
        destination: u32,
        data: Vec<u8>,
        length_bits: u16,
    },

    /// SDS report received
    SdsReport { uuid: Uuid, status: u8 },

    /// Error from server
    ServerError { error_type: u8, data: Vec<u8> },
}

/// Commands the BrewEntity sends to the worker
#[derive(Debug)]
pub enum BrewCommand {
    /// Register a subscriber (ISSI)
    RegisterSubscriber { issi: u32 },

    /// Deregister a subscriber (ISSI)
    DeregisterSubscriber { issi: u32 },

    /// Affiliate subscriber to groups
    AffiliateGroups { issi: u32, groups: Vec<u32> },

    /// Deaffiliate subscriber from groups
    DeaffiliateGroups { issi: u32, groups: Vec<u32> },

    /// Send GROUP_TX to TetraPack (local radio started transmitting on subscribed group)
    SendGroupTx {
        uuid: Uuid,
        source_issi: u32,
        dest_gssi: u32,
        priority: u8,
        service: u16,
    },

    /// Send a voice frame to TetraPack (ACELP data from UL)
    SendVoiceFrame { uuid: Uuid, length_bits: u16, data: Vec<u8> },

    /// Send GROUP_IDLE to TetraPack (transmission ended)
    SendGroupIdle { uuid: Uuid, cause: u8 },

    /// Send SDS to TetraPack (SHORT_TRANSFER + SDS_TRANSFER)
    SendSds {
        uuid: Uuid,
        source: u32,
        destination: u32,
        data: Vec<u8>,
        length_bits: u16,
    },

    /// Send SDS report to Brew (delivery acknowledgement)
    SendSdsReport { uuid: Uuid, status: u8 },

    /// Disconnect gracefully
    Disconnect,
}

// ─── Worker ───────────────────────────────────────────────────────

/// Pending SDS header data (from CALL_STATE_SHORT_TRANSFER), awaiting matching FRAME_TYPE_SDS_TRANSFER
#[derive(Debug)]
struct PendingSds {
    source: u32,
    destination: u32,
    received_at: Instant,
}

/// Brew protocol worker, generic over the network transport.
///
/// Runs in a separate thread. Communicates with [`super::entity::BrewEntity`] via
/// crossbeam channels ([`BrewEvent`] and [`BrewCommand`]).
pub struct BrewWorker<T: NetworkTransport> {
    config: SharedConfig,
    brew_config: CfgBrew,
    /// Network transport (WebSocket, QUIC, TCP, …)
    transport: T,
    /// Send events to the BrewEntity
    event_sender: Sender<BrewEvent>,
    /// Receive commands from the BrewEntity
    command_receiver: Receiver<BrewCommand>,
    /// Registered subscribers and their affiliated groups (tracked from commands)
    subscriber_groups: HashMap<u32, HashSet<u32>>,
    /// Pending SDS transfers keyed by UUID, awaiting matching SDS_TRANSFER frame
    pending_sds: HashMap<Uuid, PendingSds>,
}

impl<T: NetworkTransport> BrewWorker<T> {
    pub fn new(config: SharedConfig, event_sender: Sender<BrewEvent>, command_receiver: Receiver<BrewCommand>, transport: T) -> Self {
        let brew_config = config.config().brew.clone().unwrap(); // Never fails
        Self {
            config,
            brew_config,
            transport,
            event_sender,
            command_receiver,
            subscriber_groups: HashMap::new(),
            pending_sds: HashMap::new(),
        }
    }

    /// Main worker entry point — runs until disconnect or fatal error
    pub fn run(&mut self) {
        tracing::info!("BrewWorker: starting");

        loop {
            // Attempt connection via transport
            match self.transport.connect() {
                Ok(()) => {
                    tracing::info!("BrewWorker: transport connected");
                    let _ = self.event_sender.send(BrewEvent::Connected);
                }
                Err(e) => {
                    tracing::error!(
                        "BrewWorker: connection error: {}, reconnecting in {:?}",
                        e,
                        self.brew_config.reconnect_delay
                    );
                    let _ = self.event_sender.send(BrewEvent::Disconnected(e.to_string()));
                    std::thread::sleep(self.brew_config.reconnect_delay);
                    continue;
                }
            }

            // Run the message loop until error or clean shutdown
            match self.message_loop() {
                Ok(()) => {
                    tracing::info!("BrewWorker: connection closed normally");
                    break;
                }
                Err(e) => {
                    tracing::error!(
                        "BrewWorker: connection error: {}, reconnecting in {:?}",
                        e,
                        self.brew_config.reconnect_delay
                    );
                    let _ = self.event_sender.send(BrewEvent::Disconnected(e));
                    std::thread::sleep(self.brew_config.reconnect_delay);
                }
            }
        }
    }

    /// Graceful teardown: DEAFFILIATE → DEREGISTER → transport disconnect
    fn graceful_teardown(&mut self) {
        for (issi, groups) in &self.subscriber_groups {
            if !groups.is_empty() {
                let mut group_list: Vec<u32> = groups.iter().copied().collect();
                group_list.sort_unstable();
                let deaff_msg = build_subscriber_deaffiliate(*issi, &group_list);
                if let Err(e) = self.transport.send_reliable(&deaff_msg) {
                    tracing::error!("BrewWorker: failed to send deaffiliation: {}", e);
                } else {
                    tracing::info!("BrewWorker: deaffiliated issi={} groups={:?}", issi, group_list);
                }
            }

            let dereg_msg = build_subscriber_deregister(*issi);
            if let Err(e) = self.transport.send_reliable(&dereg_msg) {
                tracing::error!("BrewWorker: failed to send deregistration: {}", e);
            } else {
                tracing::info!("BrewWorker: deregistered ISSI {}", issi);
            }
        }
        self.transport.disconnect();
    }

    /// Main message processing loop (transport-agnostic)
    fn message_loop(&mut self) -> Result<(), String> {
        loop {
            let now = Instant::now();

            // Expire stale pending SDS entries (SHORT_TRANSFER without matching SDS_TRANSFER)
            self.pending_sds.retain(|uuid, pending| {
                let age = now.duration_since(pending.received_at);
                if age > Duration::from_secs(30) {
                    tracing::warn!("BrewWorker: expiring stale pending SDS uuid={}", uuid);
                    false
                } else {
                    true
                }
            });

            // ── Receive incoming messages from transport ──
            let messages = self.transport.receive_reliable();
            for msg in messages {
                self.handle_incoming_binary(&msg.payload);
            }

            // Check if transport is still connected (may have been dropped during receive)
            if !self.transport.is_connected() {
                return Err("transport disconnected".to_string());
            }

            // ── Check for commands from the BrewEntity ──
            loop {
                let cmd = match self.command_receiver.try_recv() {
                    Ok(cmd) => cmd,
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        // Entity was dropped — do graceful teardown
                        tracing::info!("BrewWorker: command channel closed, performing graceful teardown");
                        self.graceful_teardown();
                        return Ok(());
                    }
                };
                match cmd {
                    BrewCommand::RegisterSubscriber { issi } => {
                        let already_registered = self.subscriber_groups.contains_key(&issi);
                        self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                        let msg = if already_registered {
                            build_subscriber_reregister(issi)
                        } else {
                            build_subscriber_register(issi, &[])
                        };
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send registration: {}", e);
                        } else {
                            tracing::debug!(
                                "BrewWorker: sent {} issi={}",
                                if already_registered { "REREGISTER" } else { "REGISTER" },
                                issi
                            );
                        }
                    }
                    BrewCommand::DeregisterSubscriber { issi } => {
                        self.subscriber_groups.remove(&issi);
                        let msg = build_subscriber_deregister(issi);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send deregistration: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent DEREGISTER issi={}", issi);
                        }
                    }
                    BrewCommand::AffiliateGroups { issi, groups } => {
                        let entry = self.subscriber_groups.entry(issi).or_insert_with(HashSet::new);
                        for gssi in &groups {
                            entry.insert(*gssi);
                        }
                        let msg = build_subscriber_affiliate(issi, &groups);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send affiliation: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent AFFILIATE issi={} groups={:?}", issi, groups);
                        }
                    }
                    BrewCommand::DeaffiliateGroups { issi, groups } => {
                        if let Some(entry) = self.subscriber_groups.get_mut(&issi) {
                            for gssi in &groups {
                                entry.remove(gssi);
                            }
                        }
                        let msg = build_subscriber_deaffiliate(issi, &groups);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send deaffiliation: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent DEAFFILIATE issi={} groups={:?}", issi, groups);
                        }
                    }
                    BrewCommand::SendGroupTx {
                        uuid,
                        source_issi,
                        dest_gssi,
                        priority,
                        service,
                    } => {
                        let msg = build_group_tx(&uuid, source_issi, dest_gssi, priority, service);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send GROUP_TX: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent GROUP_TX uuid={} src={} dst={}", uuid, source_issi, dest_gssi);
                        }
                    }
                    BrewCommand::SendVoiceFrame { uuid, length_bits, data } => {
                        let msg = build_voice_frame(&uuid, length_bits, &data);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send voice frame: {}", e);
                        }
                    }
                    BrewCommand::SendGroupIdle { uuid, cause } => {
                        let msg = build_group_idle(&uuid, cause);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::error!("BrewWorker: failed to send GROUP_IDLE: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent GROUP_IDLE uuid={} cause={}", uuid, cause);
                        }
                    }
                    BrewCommand::SendSds {
                        uuid,
                        source,
                        destination,
                        data,
                        length_bits,
                    } => {
                        if !brew::feature_sds_enabled(&self.config) {
                            tracing::warn!("BrewWorker: ignoring SendSds command because SDS over Brew is disabled in config");
                            continue;
                        }

                        // Send SHORT_TRANSFER first (header with source/dest)
                        let short_msg = build_short_transfer(&uuid, source, destination);
                        if let Err(e) = self.transport.send_reliable(&short_msg) {
                            tracing::error!("BrewWorker: failed to send SHORT_TRANSFER: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent SHORT_TRANSFER uuid={} src={} dst={}", uuid, source, destination);
                            // Then send SDS_TRANSFER with the payload
                            let sds_msg = build_sds_frame(&uuid, length_bits, &data);
                            if let Err(e) = self.transport.send_reliable(&sds_msg) {
                                tracing::error!("BrewWorker: failed to send SDS_TRANSFER: {}", e);
                            } else {
                                tracing::debug!("BrewWorker: sent SDS_TRANSFER uuid={} {} bytes", uuid, data.len());
                            }
                        }
                    }
                    BrewCommand::SendSdsReport { uuid, status } => {
                        if !brew::feature_sds_enabled(&self.config) {
                            tracing::warn!("BrewWorker: ignoring SendSdsReport command because SDS over Brew is disabled in config");
                            continue;
                        }

                        let msg = build_sds_report(&uuid, status);
                        if let Err(e) = self.transport.send_reliable(&msg) {
                            tracing::warn!("BrewWorker: failed to send SDS_REPORT: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent SDS_REPORT uuid={} status={}", uuid, status);
                        }
                    }
                    BrewCommand::Disconnect => {
                        self.graceful_teardown();
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Parse an incoming binary Brew message and forward as event
    fn handle_incoming_binary(&mut self, data: &[u8]) {
        match parse_brew_message(data) {
            Ok(msg) => match msg {
                BrewMessage::CallControl(cc) => self.handle_call_control(cc),
                BrewMessage::Frame(frame) => self.handle_frame(frame),
                BrewMessage::Subscriber(sub) => {
                    tracing::debug!("BrewWorker: subscriber event type={}", sub.msg_type);
                    // TODO FIXME we could check whether this call is indeed a brew ssi here
                    let _ = self.event_sender.send(BrewEvent::SubscriberEvent {
                        msg_type: sub.msg_type,
                        issi: sub.number,
                        groups: sub.groups,
                    });
                }
                BrewMessage::Error(err) => {
                    tracing::warn!("BrewWorker: server error type={}: {} bytes", err.error_type, err.data.len());
                    // TODO FIXME we could check whether this call is indeed a brew ssi here
                    let _ = self.event_sender.send(BrewEvent::ServerError {
                        error_type: err.error_type,
                        data: err.data,
                    });
                }
                BrewMessage::Service(svc) => {
                    tracing::debug!("BrewWorker: service type={}: {}", svc.service_type, svc.json_data);
                }
            },
            Err(e) => {
                tracing::warn!("BrewWorker: failed to parse message ({} bytes): {}", data.len(), e);
            }
        }
    }

    /// Handle a parsed call control message
    fn handle_call_control(&mut self, cc: BrewCallControlMessage) {
        match cc.call_state {
            CALL_STATE_GROUP_TX => {
                if let BrewCallPayload::GroupTransmission(gt) = cc.payload {
                    tracing::info!(
                        "BrewWorker: GROUP_TX uuid={} src={} dst={} prio={} service={}",
                        cc.identifier,
                        gt.source,
                        gt.destination,
                        gt.priority,
                        gt.service
                    );
                    if !brew::is_brew_gssi_routable(&self.config, gt.destination) {
                        tracing::warn!("BrewWorker: dropping GROUP_TX to non-routable GSSI {}", gt.destination);
                        return;
                    };
                    let _ = self.event_sender.send(BrewEvent::GroupCallStart {
                        uuid: cc.identifier,
                        source_issi: gt.source,
                        dest_gssi: gt.destination,
                        priority: gt.priority,
                        service: gt.service,
                    });
                }
            }
            CALL_STATE_GROUP_IDLE => {
                let cause = if let BrewCallPayload::Cause(c) = cc.payload { c } else { 0 };
                tracing::info!("BrewWorker: GROUP_IDLE uuid={} cause={}", cc.identifier, cause);
                // TODO FIXME we could check whether this call is indeed a brew call here
                let _ = self.event_sender.send(BrewEvent::GroupCallEnd {
                    uuid: cc.identifier,
                    cause,
                });
            }
            CALL_STATE_CALL_RELEASE => {
                let cause = if let BrewCallPayload::Cause(c) = cc.payload { c } else { 0 };
                tracing::info!("BrewWorker: CALL_RELEASE uuid={} cause={}", cc.identifier, cause);
                // TODO FIXME we could check whether this call is indeed a brew call here
                let _ = self.event_sender.send(BrewEvent::GroupCallEnd {
                    uuid: cc.identifier,
                    cause,
                });
            }
            CALL_STATE_SHORT_TRANSFER => {
                if let BrewCallPayload::ShortTransfer { source, destination } = cc.payload {
                    tracing::info!(
                        "BrewWorker: SHORT_TRANSFER uuid={} src={} dst={}",
                        cc.identifier,
                        source,
                        destination
                    );
                    // Stash for matching with upcoming SDS_TRANSFER frame
                    self.pending_sds.insert(
                        cc.identifier,
                        PendingSds {
                            source,
                            destination,
                            received_at: Instant::now(),
                        },
                    );
                }
            }
            state => {
                tracing::debug!("BrewWorker: unhandled call state {} uuid={}", state, cc.identifier);
            }
        }
    }

    /// Handle a parsed voice/data frame
    fn handle_frame(&mut self, frame: BrewFrameMessage) {
        match frame.frame_type {
            FRAME_TYPE_TRAFFIC_CHANNEL => {
                // Forward ACELP voice frame to entity
                // TODO FIXME we could check whether this call is indeed a brew call here
                let _ = self.event_sender.send(BrewEvent::VoiceFrame {
                    uuid: frame.identifier,
                    length_bits: frame.length_bits,
                    data: frame.data,
                });
            }
            FRAME_TYPE_SDS_TRANSFER => {
                if !brew::feature_sds_enabled(&self.config) {
                    tracing::warn!("BrewWorker: ignoring incoming SDS_TRANSFER because SDS over Brew is disabled in config");
                    return;
                }

                if frame.length_bits > 2047 {
                    // TODO FIXME we could split into multiple SDS messages here
                    tracing::warn!(
                        "BrewWorker: ignoring SDS_TRANSFER with excessive length_bits={} ({} bytes)",
                        frame.length_bits,
                        frame.data.len()
                    );
                    return;
                }

                // Match with pending SHORT_TRANSFER by UUID
                if let Some(pending) = self.pending_sds.remove(&frame.identifier) {
                    tracing::info!(
                        "BrewWorker: SDS_TRANSFER uuid={} src={} dst={} {} bytes",
                        frame.identifier,
                        pending.source,
                        pending.destination,
                        frame.data.len()
                    );
                    let _ = self.event_sender.send(BrewEvent::SdsTransfer {
                        uuid: frame.identifier,
                        source: pending.source,
                        destination: pending.destination,
                        data: frame.data,
                        length_bits: frame.length_bits,
                    });
                } else {
                    tracing::warn!(
                        "BrewWorker: SDS_TRANSFER uuid={} without matching SHORT_TRANSFER, {} bytes",
                        frame.identifier,
                        frame.data.len()
                    );
                }
            }
            FRAME_TYPE_SDS_REPORT => {
                let status = if frame.data.is_empty() { 0 } else { frame.data[0] };
                tracing::debug!("BrewWorker: SDS_REPORT uuid={} status={}", frame.identifier, status);
                let _ = self.event_sender.send(BrewEvent::SdsReport {
                    uuid: frame.identifier,
                    status,
                });
            }
            ft => {
                tracing::debug!("BrewWorker: unhandled frame type {} uuid={}", ft, frame.identifier);
            }
        }
    }
}
