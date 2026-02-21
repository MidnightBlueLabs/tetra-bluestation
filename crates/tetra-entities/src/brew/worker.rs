//! Brew WebSocket worker thread handling HTTP Digest Auth, TLS, and bidirectional Brew message exchange

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};
use uuid::Uuid;

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

    /// Error from server
    ServerError { error_type: u8, data: Vec<u8> },
}

/// Commands the BrewEntity sends to the worker
#[derive(Debug)]
pub enum BrewCommand {
    /// Register a subscriber (ISSI)
    RegisterSubscriber { issi: u32 },

    /// Affiliate subscriber to groups
    AffiliateGroups { issi: u32, groups: Vec<u32> },

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

    /// Disconnect gracefully
    Disconnect,
}

// ─── Configuration ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BrewConfig {
    /// TetraPack server hostname or IP
    pub host: String,
    /// TetraPack server port
    pub port: u16,
    /// Use TLS (wss:// / https://)
    pub tls: bool,
    /// Optional username for HTTP Digest auth
    pub username: Option<String>,
    /// Optional password for HTTP Digest auth
    pub password: Option<String>,
    /// ISSI to register with the server
    pub issi: u32,
    /// GSSIs (group IDs) to affiliate to
    pub groups: Vec<u32>,
    /// Reconnection delay
    pub reconnect_delay: Duration,
    /// Extra initial jitter playout delay in frames (added on top of adaptive baseline)
    pub jitter_initial_latency_frames: u8,
}

// ─── TLS helper ──────────────────────────────────────────────────

/// A stream that is either plain TCP or TLS-wrapped TCP
enum BrewStream {
    Plain(TcpStream),
    Tls(rustls::StreamOwned<rustls::ClientConnection, TcpStream>),
}

impl Read for BrewStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            BrewStream::Plain(s) => s.read(buf),
            BrewStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for BrewStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            BrewStream::Plain(s) => s.write(buf),
            BrewStream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            BrewStream::Plain(s) => s.flush(),
            BrewStream::Tls(s) => s.flush(),
        }
    }
}

/// Build a rustls ClientConfig with system root certificates
fn build_tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let mut root_store = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().map_err(|e| format!("load certs: {}", e))? {
        let _ = root_store.add(cert);
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Connect a TCP stream, optionally wrapping with TLS
fn connect_stream(host: &str, port: u16, use_tls: bool) -> Result<BrewStream, String> {
    let addr = format!("{}:{}", host, port);
    tracing::debug!("BrewWorker: connecting TCP to {}", addr);

    let socket_addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolve failed for '{}': {}", addr, e))?
        .next()
        .ok_or_else(|| format!("no addresses found for '{}'", addr))?;

    tracing::debug!("BrewWorker: resolved {} -> {}", addr, socket_addr);

    let tcp = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(10)).map_err(|e| format!("TCP connect failed: {}", e))?;

    tcp.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| format!("set read timeout: {}", e))?;

    if use_tls {
        let tls_config = build_tls_config()?;
        let server_name: rustls::pki_types::ServerName<'static> = host
            .to_string()
            .try_into()
            .map_err(|e| format!("invalid server name '{}': {}", host, e))?;
        let tls_conn = rustls::ClientConnection::new(tls_config, server_name).map_err(|e| format!("TLS init failed: {}", e))?;
        let tls_stream = rustls::StreamOwned::new(tls_conn, tcp);
        tracing::debug!("BrewWorker: TLS connected to {}", addr);
        Ok(BrewStream::Tls(tls_stream))
    } else {
        Ok(BrewStream::Plain(tcp))
    }
}

// ─── HTTP Digest Auth helpers ────────────────────────────────────

/// Compute MD5 hex digest of a string
fn md5_hex(input: &str) -> String {
    let digest = md5::compute(input.as_bytes());
    format!("{:x}", digest)
}

/// Parse a "Digest realm=..., nonce=..., ..." challenge into key-value pairs
fn parse_digest_challenge(header: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    // Strip "Digest " prefix
    let s = header.strip_prefix("Digest ").unwrap_or(header);
    for part in s.split(',') {
        let part = part.trim();
        if let Some(eq) = part.find('=') {
            let key = part[..eq].trim().to_lowercase();
            let val = part[eq + 1..].trim().trim_matches('"').to_string();
            params.insert(key, val);
        }
    }
    params
}

/// Build an Authorization header for HTTP Digest Auth
fn build_digest_response(
    username: &str,
    password: &str,
    realm: &str,
    nonce: &str,
    qop: &str,
    uri: &str,
    method: &str,
    opaque: Option<&str>,
) -> String {
    let ha1 = md5_hex(&format!("{}:{}:{}", username, realm, password));
    let ha2 = md5_hex(&format!("{}:{}", method, uri));

    let nc = "00000001";
    let cnonce = format!(
        "{:08x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    );

    let response_hash = if qop.contains("auth") {
        md5_hex(&format!("{}:{}:{}:{}:{}:{}", ha1, nonce, nc, cnonce, "auth", ha2))
    } else {
        md5_hex(&format!("{}:{}:{}", ha1, nonce, ha2))
    };

    let mut auth = format!(
        "Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\"",
        username, realm, nonce, uri, response_hash
    );
    if qop.contains("auth") {
        auth.push_str(&format!(", qop=auth, nc={}, cnonce=\"{}\"", nc, cnonce));
    }
    if let Some(opaque_val) = opaque {
        auth.push_str(&format!(", opaque=\"{}\"", opaque_val));
    }
    auth
}

// ─── Worker ───────────────────────────────────────────────────────

pub struct BrewWorker {
    config: BrewConfig,
    /// Send events to the BrewEntity
    event_sender: Sender<BrewEvent>,
    /// Receive commands from the BrewEntity
    command_receiver: Receiver<BrewCommand>,
}

impl BrewWorker {
    pub fn new(config: BrewConfig, event_sender: Sender<BrewEvent>, command_receiver: Receiver<BrewCommand>) -> Self {
        Self {
            config,
            event_sender,
            command_receiver,
        }
    }

    /// Main worker entry point — runs until disconnect or fatal error
    pub fn run(&mut self) {
        let scheme = if self.config.tls { "wss" } else { "ws" };
        tracing::info!("BrewWorker starting, server {}://{}:{}", scheme, self.config.host, self.config.port);

        loop {
            // Attempt connection
            match self.connect_and_run() {
                Ok(()) => {
                    tracing::info!("BrewWorker: connection closed normally");
                    break;
                }
                Err(e) => {
                    tracing::error!("BrewWorker: connection error: {}", e);
                    let _ = self.event_sender.send(BrewEvent::Disconnected(e.clone()));
                    tracing::info!("BrewWorker: reconnecting in {:?}", self.config.reconnect_delay);
                    std::thread::sleep(self.config.reconnect_delay);
                }
            }
        }

        tracing::info!("BrewWorker stopped");
    }

    fn user_agent() -> String {
        format!("BlueStation/{}", tetra_core::STACK_VERSION)
    }

    /// Perform HTTP GET /brew/ with optional Digest Auth to get the WebSocket endpoint
    fn authenticate(&self) -> Result<String, String> {
        let host = &self.config.host;
        let port = self.config.port;

        // ── First request (unauthenticated) ──
        let mut stream = connect_stream(host, port, self.config.tls)?;

        let request = format!(
            "GET /brew/ HTTP/1.1\r\n\
             Host: {}\r\n\
             User-Agent: {}\r\n\
             \r\n",
            host,
            Self::user_agent()
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| format!("HTTP write failed: {}", e))?;

        let mut response_buf = vec![0u8; 4096];
        let n = stream.read(&mut response_buf).map_err(|e| format!("HTTP read failed: {}", e))?;

        if n == 0 {
            return Err("empty HTTP response".to_string());
        }

        let response = String::from_utf8_lossy(&response_buf[..n]).to_string();
        tracing::debug!("BrewWorker: HTTP response:\n{}", response.trim());

        let lines: Vec<&str> = response.split("\r\n").collect();
        if lines.is_empty() {
            return Err("malformed HTTP response".to_string());
        }

        let status_line = lines[0];

        // ── Handle 200 OK ──
        if status_line.contains("200") {
            return self.extract_endpoint(&response);
        }

        // ── Handle 401 Unauthorized → Digest Auth ──
        if status_line.contains("401") {
            tracing::info!("BrewWorker: server requires Digest Auth (401)");

            // Find WWW-Authenticate header
            let www_auth = lines
                .iter()
                .find(|l| l.to_lowercase().starts_with("www-authenticate"))
                .ok_or("401 but no WWW-Authenticate header")?;

            let challenge = www_auth.splitn(2, ':').nth(1).ok_or("malformed WWW-Authenticate")?.trim();

            if !challenge.to_lowercase().starts_with("digest") {
                return Err(format!("unsupported auth scheme: {}", challenge));
            }

            let (username, password) = match (&self.config.username, &self.config.password) {
                (Some(u), Some(p)) => (u.as_str(), p.as_str()),
                _ => {
                    return Err("server requires auth but no username/password configured".to_string());
                }
            };

            let params = parse_digest_challenge(challenge);
            let realm = params.get("realm").map(|s| s.as_str()).unwrap_or("");
            let nonce = params.get("nonce").map(|s| s.as_str()).unwrap_or("");
            let qop = params.get("qop").map(|s| s.as_str()).unwrap_or("");
            let opaque = params.get("opaque").map(|s| s.as_str());

            tracing::debug!("BrewWorker: digest realm={} qop={}", realm, qop);

            let auth_header = build_digest_response(username, password, realm, nonce, qop, "/brew/", "GET", opaque);

            // ── Second request (authenticated) ──
            // Drop old stream, open new connection
            drop(stream);
            let mut stream2 = connect_stream(host, port, self.config.tls)?;

            let auth_request = format!(
                "GET /brew/ HTTP/1.1\r\n\
                 Host: {}\r\n\
                 User-Agent: {}\r\n\
                 Authorization: {}\r\n\
                 \r\n",
                host,
                Self::user_agent(),
                auth_header
            );
            stream2
                .write_all(auth_request.as_bytes())
                .map_err(|e| format!("auth HTTP write failed: {}", e))?;

            let mut auth_buf = vec![0u8; 4096];
            let n2 = stream2.read(&mut auth_buf).map_err(|e| format!("auth HTTP read failed: {}", e))?;

            if n2 == 0 {
                return Err("empty auth HTTP response".to_string());
            }

            let auth_response = String::from_utf8_lossy(&auth_buf[..n2]).to_string();
            tracing::debug!("BrewWorker: auth response:\n{}", auth_response.trim());

            let auth_status = auth_response.split("\r\n").next().unwrap_or("");

            if auth_status.contains("200") {
                return self.extract_endpoint(&auth_response);
            }

            return Err(format!("authentication failed: {}", auth_status));
        }

        Err(format!("unexpected HTTP status: {}", status_line))
    }

    /// Extract the endpoint path from a 200 OK response body
    fn extract_endpoint(&self, response: &str) -> Result<String, String> {
        let body_start = response.find("\r\n\r\n");
        if let Some(pos) = body_start {
            let endpoint = response[pos + 4..].trim().to_string();
            if endpoint.starts_with('/') {
                tracing::info!("BrewWorker: got endpoint: {}", endpoint);
                return Ok(endpoint);
            }
            return Err(format!("invalid endpoint path: {}", endpoint));
        }
        Err("no body in 200 response".to_string())
    }

    /// Connect to the server and run the message loop
    fn connect_and_run(&mut self) -> Result<(), String> {
        // Step 1: HTTP auth to get WebSocket endpoint
        let endpoint = self.authenticate()?;

        // Step 2: Connect WebSocket to the endpoint
        let scheme = if self.config.tls { "wss" } else { "ws" };
        let ws_url = format!("{}://{}:{}{}", scheme, self.config.host, self.config.port, endpoint);
        tracing::info!("BrewWorker: connecting WebSocket to {}", ws_url);

        // Build request with User-Agent and subprotocol headers.
        // The TetraPack server sends a Sec-WebSocket-Protocol in its response,
        // so we must request one to satisfy the RFC 6455 handshake validation.
        let request = tungstenite::http::Request::builder()
            .uri(&ws_url)
            .header("Host", format!("{}:{}", self.config.host, self.config.port))
            .header("User-Agent", Self::user_agent())
            .header("Sec-WebSocket-Protocol", "brew")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Key", tungstenite::handshake::client::generate_key())
            .header("Sec-WebSocket-Version", "13")
            .body(())
            .map_err(|e| format!("failed to build WS request: {}", e))?;

        let (mut ws, _response) = tungstenite::connect(request).map_err(|e| format!("WebSocket connect failed: {}", e))?;

        tracing::info!("BrewWorker: WebSocket connected");
        let _ = self.event_sender.send(BrewEvent::Connected);

        // Set non-blocking for polling and TCP_NODELAY as recommended
        match ws.get_ref() {
            MaybeTlsStream::Plain(stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(10)));
                let _ = stream.set_nodelay(true);
            }
            MaybeTlsStream::Rustls(tls_stream) => {
                let tcp = tls_stream.get_ref();
                let _ = tcp.set_read_timeout(Some(Duration::from_millis(10)));
                let _ = tcp.set_nodelay(true);
            }
            _ => {}
        }

        // Step 3: Register subscriber and affiliate to groups
        self.send_registration(&mut ws)?;

        // Step 4: Main message loop
        self.message_loop(&mut ws)
    }

    /// Send initial registration and group affiliation
    fn send_registration(&self, ws: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> Result<(), String> {
        // Register ISSI
        let reg_msg = build_subscriber_register(self.config.issi, &self.config.groups);
        ws.send(Message::Binary(reg_msg.into()))
            .map_err(|e| format!("failed to send registration: {}", e))?;
        tracing::info!("BrewWorker: registered ISSI {}", self.config.issi);

        // Affiliate to groups
        if !self.config.groups.is_empty() {
            let aff_msg = build_subscriber_affiliate(self.config.issi, &self.config.groups);
            ws.send(Message::Binary(aff_msg.into()))
                .map_err(|e| format!("failed to send affiliation: {}", e))?;
            tracing::info!("BrewWorker: affiliated to groups {:?}", self.config.groups);
        }

        Ok(())
    }

    /// Graceful teardown: DEAFFILIATE → DEREGISTER → WS close
    fn graceful_teardown(&self, ws: &mut WebSocket<MaybeTlsStream<TcpStream>>) {
        if !self.config.groups.is_empty() {
            let deaff_msg = build_subscriber_deaffiliate(self.config.issi, &self.config.groups);
            if let Err(e) = ws.send(Message::Binary(deaff_msg.into())) {
                tracing::error!("BrewWorker: failed to send deaffiliation: {}", e);
            } else {
                tracing::info!("BrewWorker: deaffiliated from groups {:?}", self.config.groups);
            }
        }
        let dereg_msg = build_subscriber_deregister(self.config.issi);
        if let Err(e) = ws.send(Message::Binary(dereg_msg.into())) {
            tracing::error!("BrewWorker: failed to send deregistration: {}", e);
        } else {
            tracing::info!("BrewWorker: deregistered ISSI {}", self.config.issi);
        }
        let _ = ws.close(None);
    }

    /// Main WebSocket message processing loop
    fn message_loop(&mut self, ws: &mut WebSocket<MaybeTlsStream<TcpStream>>) -> Result<(), String> {
        loop {
            // ── Check for incoming WebSocket messages ──
            match ws.read() {
                Ok(Message::Binary(data)) => {
                    self.handle_incoming_binary(&data);
                }
                Ok(Message::Ping(payload)) => {
                    let _ = ws.send(Message::Pong(payload));
                }
                Ok(Message::Pong(_)) => {
                    // Latency measurement — ignore for now
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("BrewWorker: server sent close");
                    return Ok(());
                }
                Ok(_) => {
                    // Text or other — unexpected for Brew
                }
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // No data available — normal for non-blocking
                }
                Err(tungstenite::Error::ConnectionClosed) => {
                    return Err("connection closed by server".to_string());
                }
                Err(e) => {
                    return Err(format!("WebSocket read error: {}", e));
                }
            }

            // ── Check for commands from the BrewEntity ──
            loop {
                let cmd = match self.command_receiver.try_recv() {
                    Ok(cmd) => cmd,
                    Err(crossbeam_channel::TryRecvError::Empty) => break,
                    Err(crossbeam_channel::TryRecvError::Disconnected) => {
                        // Entity was dropped — do graceful teardown
                        tracing::info!("BrewWorker: command channel closed, performing graceful teardown");
                        self.graceful_teardown(ws);
                        return Ok(());
                    }
                };
                match cmd {
                    BrewCommand::RegisterSubscriber { issi } => {
                        let msg = build_subscriber_register(issi, &[]);
                        if let Err(e) = ws.send(Message::Binary(msg.into())) {
                            tracing::error!("BrewWorker: failed to send registration: {}", e);
                        }
                    }
                    BrewCommand::AffiliateGroups { issi, groups } => {
                        let msg = build_subscriber_affiliate(issi, &groups);
                        if let Err(e) = ws.send(Message::Binary(msg.into())) {
                            tracing::error!("BrewWorker: failed to send affiliation: {}", e);
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
                        if let Err(e) = ws.send(Message::Binary(msg.into())) {
                            tracing::error!("BrewWorker: failed to send GROUP_TX: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent GROUP_TX uuid={} src={} dst={}", uuid, source_issi, dest_gssi);
                        }
                    }
                    BrewCommand::SendVoiceFrame { uuid, length_bits, data } => {
                        let msg = build_voice_frame(&uuid, length_bits, &data);
                        if let Err(e) = ws.send(Message::Binary(msg.into())) {
                            tracing::error!("BrewWorker: failed to send voice frame: {}", e);
                        }
                    }
                    BrewCommand::SendGroupIdle { uuid, cause } => {
                        let msg = build_group_idle(&uuid, cause);
                        if let Err(e) = ws.send(Message::Binary(msg.into())) {
                            tracing::error!("BrewWorker: failed to send GROUP_IDLE: {}", e);
                        } else {
                            tracing::debug!("BrewWorker: sent GROUP_IDLE uuid={} cause={}", uuid, cause);
                        }
                    }
                    BrewCommand::Disconnect => {
                        self.graceful_teardown(ws);
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Parse an incoming binary Brew message and forward as event
    fn handle_incoming_binary(&self, data: &[u8]) {
        match parse_brew_message(data) {
            Ok(msg) => match msg {
                BrewMessage::CallControl(cc) => self.handle_call_control(cc),
                BrewMessage::Frame(frame) => self.handle_frame(frame),
                BrewMessage::Subscriber(sub) => {
                    tracing::debug!("BrewWorker: subscriber event type={}", sub.msg_type);
                    let _ = self.event_sender.send(BrewEvent::SubscriberEvent {
                        msg_type: sub.msg_type,
                        issi: sub.number,
                        groups: sub.groups,
                    });
                }
                BrewMessage::Error(err) => {
                    tracing::warn!("BrewWorker: server error type={}: {} bytes", err.error_type, err.data.len());
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
    fn handle_call_control(&self, cc: BrewCallControlMessage) {
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
                let _ = self.event_sender.send(BrewEvent::GroupCallEnd {
                    uuid: cc.identifier,
                    cause,
                });
            }
            CALL_STATE_CALL_RELEASE => {
                let cause = if let BrewCallPayload::Cause(c) = cc.payload { c } else { 0 };
                tracing::info!("BrewWorker: CALL_RELEASE uuid={} cause={}", cc.identifier, cause);
                let _ = self.event_sender.send(BrewEvent::GroupCallEnd {
                    uuid: cc.identifier,
                    cause,
                });
            }
            state => {
                tracing::debug!("BrewWorker: unhandled call state {} uuid={}", state, cc.identifier);
            }
        }
    }

    /// Handle a parsed voice/data frame
    fn handle_frame(&self, frame: BrewFrameMessage) {
        match frame.frame_type {
            FRAME_TYPE_TRAFFIC_CHANNEL => {
                // Forward ACELP voice frame to entity
                let _ = self.event_sender.send(BrewEvent::VoiceFrame {
                    uuid: frame.identifier,
                    length_bits: frame.length_bits,
                    data: frame.data,
                });
            }
            FRAME_TYPE_SDS_TRANSFER => {
                tracing::debug!(
                    "BrewWorker: SDS transfer uuid={} {} bytes (not yet handled)",
                    frame.identifier,
                    frame.data.len()
                );
            }
            FRAME_TYPE_SDS_REPORT => {
                tracing::debug!("BrewWorker: SDS report uuid={}", frame.identifier);
            }
            ft => {
                tracing::debug!("BrewWorker: unhandled frame type {} uuid={}", ft, frame.identifier);
            }
        }
    }
}
