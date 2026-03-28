//! WebSocket transport implementation with HTTP Digest Auth, TLS, and heartbeat management
//!
//! Implements the [`NetworkTransport`] trait over WebSocket (RFC 6455), with optional
//! TLS and HTTP Digest Authentication. Heartbeat ping/pong and timeout detection are
//! managed internally by the transport, transparent to the caller.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine as _;

use tetra_config::bluestation::SecretField;
use tungstenite::{Connector, Message, WebSocket, stream::MaybeTlsStream};

use super::{NetworkAddress, NetworkError, NetworkMessage, NetworkTransport};

const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);

// ─── Configuration ────────────────────────────────────────────────

/// Configuration for the WebSocket transport
#[derive(Clone)]
pub struct WebSocketTransportConfig {
    /// Server hostname or IP
    pub host: String,
    /// Server port
    pub port: u16,
    /// Use TLS (wss://)
    pub use_tls: bool,
    /// Optional custom root certificates (DER-encoded) for TLS server validation.
    /// Can be present only when use_tls is true.
    /// When `Some`, these replace the system certificate store — useful for
    /// self-signed certificates. When `None`, the system store is used.
    pub custom_root_certs: Option<Vec<rustls::pki_types::CertificateDer<'static>>>,
    /// Optional credentials (username, password) for HTTP Basic Auth.
    /// When `Some`, an `Authorization: Basic` header is added to the WebSocket
    /// upgrade request. Used for telemetry authentication.
    pub basic_auth_credentials: Option<(String, String)>,
    /// Optional credentials (username, password) for HTTP Digest Auth.
    /// When `Some`, the transport performs HTTP auth discovery before upgrading
    /// to WebSocket. When `None`, it connects directly to `endpoint_path`.
    pub digest_auth_credentials: Option<(String, SecretField)>,

    /// HTTP path used for initial authentication request (e.g. "/brew/")
    pub endpoint_path: String,
    /// WebSocket subprotocol to negotiate (optional, e.g. "brew")
    pub subprotocol: Option<String>,
    /// User-Agent header value
    pub user_agent: String,
    /// Interval between heartbeat pings
    pub heartbeat_interval: Duration,
    /// Timeout for heartbeat (disconnect if no activity within this duration)
    pub heartbeat_timeout: Duration,
}

// ─── TLS and stream helpers ───────────────────────────────────────

/// A stream that is either plain TCP or TLS-wrapped TCP (used for authentication requests)
enum AuthStream {
    Plain(TcpStream),
    Tls(rustls::StreamOwned<rustls::ClientConnection, TcpStream>),
}

impl Read for AuthStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            AuthStream::Plain(s) => s.read(buf),
            AuthStream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for AuthStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            AuthStream::Plain(s) => s.write(buf),
            AuthStream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            AuthStream::Plain(s) => s.flush(),
            AuthStream::Tls(s) => s.flush(),
        }
    }
}

/// Build a rustls ClientConfig.
///
/// When `custom_root_certs` is `Some`, the provided DER-encoded certificates are
/// used as root trust anchors (replacing the system store). Otherwise the
/// platform's native certificate store is loaded.
fn build_tls_config(
    custom_root_certs: &Option<Vec<rustls::pki_types::CertificateDer<'static>>>,
) -> Result<Arc<rustls::ClientConfig>, String> {
    let mut root_store = rustls::RootCertStore::empty();
    match custom_root_certs {
        Some(certs) => {
            for cert in certs {
                root_store.add(cert.clone()).map_err(|e| format!("add custom cert: {}", e))?;
            }
        }
        None => {
            for cert in rustls_native_certs::load_native_certs().map_err(|e| format!("load certs: {}", e))? {
                let _ = root_store.add(cert);
            }
        }
    }
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(Arc::new(config))
}

/// Connect a TCP stream, optionally wrapping with TLS (used for HTTP auth requests)
fn connect_auth_stream(
    host: &str,
    port: u16,
    use_tls: bool,
    custom_root_certs: &Option<Vec<rustls::pki_types::CertificateDer<'static>>>,
) -> Result<AuthStream, String> {
    let addr = format!("{}:{}", host, port);
    tracing::debug!("WebSocketTransport: connecting TCP to {}", addr);

    let socket_addr = addr
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolve failed for '{}': {}", addr, e))?
        .next()
        .ok_or_else(|| format!("no addresses found for '{}'", addr))?;

    tracing::debug!("WebSocketTransport: resolved {} -> {}", addr, socket_addr);

    let tcp = TcpStream::connect_timeout(&socket_addr, DEFAULT_CONNECT_TIMEOUT).map_err(|e| format!("TCP connect failed: {}", e))?;

    tcp.set_read_timeout(Some(DEFAULT_READ_TIMEOUT))
        .map_err(|e| format!("set read timeout: {}", e))?;

    if use_tls {
        let tls_config = build_tls_config(custom_root_certs)?;
        let server_name: rustls::pki_types::ServerName<'static> = host
            .to_string()
            .try_into()
            .map_err(|e| format!("invalid server name '{}': {}", host, e))?;
        let tls_conn = rustls::ClientConnection::new(tls_config, server_name).map_err(|e| format!("TLS init failed: {}", e))?;
        let tls_stream = rustls::StreamOwned::new(tls_conn, tcp);
        tracing::debug!("WebSocketTransport: TLS connected to {}", addr);
        Ok(AuthStream::Tls(tls_stream))
    } else {
        Ok(AuthStream::Plain(tcp))
    }
}

// ─── HTTP Digest Auth helpers ─────────────────────────────────────

/// Compute MD5 hex digest of a string
fn md5_hex(input: &str) -> String {
    let digest = md5::compute(input.as_bytes());
    format!("{:x}", digest)
}

/// Parse a "Digest realm=..., nonce=..., ..." challenge into key-value pairs
fn parse_digest_challenge(header: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
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

// ─── WebSocket Transport ──────────────────────────────────────────

pub struct WebSocketTransport {
    config: WebSocketTransportConfig,
    ws: Option<WebSocket<MaybeTlsStream<TcpStream>>>,
    last_activity_at: Instant,
    last_ping_at: Instant,
    last_ping_sent_at: Option<Instant>,
    last_ping_id: Option<u64>,
    ping_seq: u64,
}

impl WebSocketTransport {
    pub fn new(config: WebSocketTransportConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            ws: None,
            last_activity_at: now,
            last_ping_at: now,
            last_ping_sent_at: None,
            last_ping_id: None,
            ping_seq: 0,
        }
    }

    /// Perform HTTP GET with optional Digest Auth to discover the WebSocket endpoint path
    fn authenticate(&self) -> Result<String, NetworkError> {
        let host = &self.config.host;
        let port = self.config.port;
        let endpoint_path = &self.config.endpoint_path;

        // ── First request (unauthenticated) ──
        let mut stream = connect_auth_stream(host, port, self.config.use_tls, &self.config.custom_root_certs)
            .map_err(|e| NetworkError::ConnectionFailed(e))?;

        let request = format!(
            "GET {} HTTP/1.1\r\n\
             Host: {}\r\n\
             User-Agent: {}\r\n\
             \r\n",
            endpoint_path, host, self.config.user_agent
        );
        stream
            .write_all(request.as_bytes())
            .map_err(|e| NetworkError::ConnectionFailed(format!("HTTP write failed: {}", e)))?;

        let mut response_buf = vec![0u8; 4096];
        let n = stream
            .read(&mut response_buf)
            .map_err(|e| NetworkError::ConnectionFailed(format!("HTTP read failed: {}", e)))?;

        if n == 0 {
            return Err(NetworkError::ConnectionFailed("empty HTTP response".to_string()));
        }

        let response = String::from_utf8_lossy(&response_buf[..n]).to_string();
        tracing::debug!("WebSocketTransport: HTTP response:\n{}", response.trim());

        let lines: Vec<&str> = response.split("\r\n").collect();
        if lines.is_empty() {
            return Err(NetworkError::ConnectionFailed("malformed HTTP response".to_string()));
        }

        let status_line = lines[0];

        // ── Handle 200 OK ──
        if status_line.contains("200") {
            return self.extract_endpoint(&response);
        }

        // ── Handle 401 Unauthorized → Digest Auth ──
        if status_line.contains("401") {
            tracing::debug!("WebSocketTransport: server requires Digest Auth (401)");

            let www_auth = lines
                .iter()
                .find(|l| l.to_lowercase().starts_with("www-authenticate"))
                .ok_or_else(|| NetworkError::ConnectionFailed("401 but no WWW-Authenticate header".to_string()))?;

            let challenge = www_auth
                .splitn(2, ':')
                .nth(1)
                .ok_or_else(|| NetworkError::ConnectionFailed("malformed WWW-Authenticate".to_string()))?
                .trim();

            if !challenge.to_lowercase().starts_with("digest") {
                return Err(NetworkError::ConnectionFailed(format!("unsupported auth scheme: {}", challenge)));
            }

            let (username, password) = match &self.config.digest_auth_credentials {
                Some((u, p)) => (u.as_str(), p.as_ref()),
                None => {
                    return Err(NetworkError::ConnectionFailed(
                        "server requires auth but no credentials configured".to_string(),
                    ));
                }
            };

            let params = parse_digest_challenge(challenge);
            let realm = params.get("realm").map(|s| s.as_str()).unwrap_or("");
            let nonce = params.get("nonce").map(|s| s.as_str()).unwrap_or("");
            let qop = params.get("qop").map(|s| s.as_str()).unwrap_or("");
            let opaque = params.get("opaque").map(|s| s.as_str());

            tracing::debug!("WebSocketTransport: digest realm={} qop={}", realm, qop);

            let auth_header = build_digest_response(username, password, realm, nonce, qop, endpoint_path, "GET", opaque);

            // ── Second request (authenticated) ──
            drop(stream);
            let mut stream2 = connect_auth_stream(host, port, self.config.use_tls, &self.config.custom_root_certs)
                .map_err(|e| NetworkError::ConnectionFailed(e))?;

            let auth_request = format!(
                "GET {} HTTP/1.1\r\n\
                 Host: {}\r\n\
                 User-Agent: {}\r\n\
                 Authorization: {}\r\n\
                 \r\n",
                endpoint_path, host, self.config.user_agent, auth_header
            );
            stream2
                .write_all(auth_request.as_bytes())
                .map_err(|e| NetworkError::ConnectionFailed(format!("auth HTTP write failed: {}", e)))?;

            let mut auth_buf = vec![0u8; 4096];
            let n2 = stream2
                .read(&mut auth_buf)
                .map_err(|e| NetworkError::ConnectionFailed(format!("auth HTTP read failed: {}", e)))?;

            if n2 == 0 {
                return Err(NetworkError::ConnectionFailed("empty auth HTTP response".to_string()));
            }

            let auth_response = String::from_utf8_lossy(&auth_buf[..n2]).to_string();
            tracing::debug!("WebSocketTransport: auth response:\n{}", auth_response.trim());

            let auth_status = auth_response.split("\r\n").next().unwrap_or("");

            if auth_status.contains("200") {
                return self.extract_endpoint(&auth_response);
            }

            return Err(NetworkError::ConnectionFailed(format!("authentication failed: {}", auth_status)));
        }

        Err(NetworkError::ConnectionFailed(format!("unexpected HTTP status: {}", status_line)))
    }

    /// Extract the endpoint path from a 200 OK response body
    fn extract_endpoint(&self, response: &str) -> Result<String, NetworkError> {
        let body_start = response.find("\r\n\r\n");
        if let Some(pos) = body_start {
            let endpoint = response[pos + 4..].trim().to_string();
            if endpoint.starts_with('/') {
                tracing::debug!("WebSocketTransport: got endpoint: {}", endpoint);
                return Ok(endpoint);
            }
            return Err(NetworkError::ConnectionFailed(format!("invalid endpoint path: {}", endpoint)));
        }
        Err(NetworkError::ConnectionFailed("no body in 200 response".to_string()))
    }
}

impl NetworkTransport for WebSocketTransport {
    fn connect(&mut self) -> Result<(), NetworkError> {
        // Drop any existing connection
        self.ws = None;

        let scheme = if self.config.use_tls { "wss" } else { "ws" };
        tracing::debug!(
            "WebSocketTransport: connecting to {}://{}:{}",
            scheme,
            self.config.host,
            self.config.port
        );

        // Step 1: Resolve WebSocket endpoint path
        let endpoint = if self.config.digest_auth_credentials.is_some() {
            self.authenticate()?
        } else {
            self.config.endpoint_path.clone()
        };

        // Step 2: Connect WebSocket to the endpoint
        let ws_url = format!("{}://{}:{}{}", scheme, self.config.host, self.config.port, endpoint);
        tracing::debug!("WebSocketTransport: connecting WebSocket to {}", ws_url);

        // Build request with User-Agent and subprotocol headers.
        // The TetraPack server sends a Sec-WebSocket-Protocol in its response,
        // so we must request one to satisfy the RFC 6455 handshake validation.
        let websocket_key = tungstenite::handshake::client::generate_key();
        let mut builder = tungstenite::http::Request::builder()
            .uri(&ws_url)
            .header("Host", format!("{}:{}", self.config.host, self.config.port))
            .header("User-Agent", &self.config.user_agent)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Key", websocket_key)
            .header("Sec-WebSocket-Version", "13");

        if let Some(ref proto) = self.config.subprotocol {
            builder = builder.header("Sec-WebSocket-Protocol", proto.as_str());
        }

        if let Some((ref user, ref pass)) = self.config.basic_auth_credentials {
            let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user, pass));
            builder = builder.header("Authorization", format!("Basic {}", encoded));
        }

        let request = builder
            .body(())
            .map_err(|e| NetworkError::ConnectionFailed(format!("failed to build WS request: {}", e)))?;

        // Open TCP stream and perform the WebSocket handshake with an optional
        // custom TLS connector (for self-signed certificate support).
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let socket_addr = addr
            .to_socket_addrs()
            .map_err(|e| NetworkError::ConnectionFailed(format!("DNS resolve failed for '{}': {}", addr, e)))?
            .next()
            .ok_or_else(|| NetworkError::ConnectionFailed(format!("no addresses found for '{}'", addr)))?;
        let tcp = TcpStream::connect_timeout(&socket_addr, DEFAULT_CONNECT_TIMEOUT)
            .map_err(|e| NetworkError::ConnectionFailed(format!("TCP connect failed: {}", e)))?;

        let connector = if self.config.custom_root_certs.is_some() {
            let tls_config = build_tls_config(&self.config.custom_root_certs).map_err(|e| NetworkError::ConnectionFailed(e))?;
            Some(Connector::Rustls(tls_config))
        } else {
            None
        };

        let (ws, _response) = tungstenite::client_tls_with_config(request, tcp, None, connector)
            .map_err(|e| NetworkError::ConnectionFailed(format!("WebSocket connect failed: {}", e)))?;

        tracing::debug!("WebSocketTransport: WebSocket connected");

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

        let now = Instant::now();
        self.ws = Some(ws);
        self.last_activity_at = now;
        self.last_ping_at = now;
        self.ping_seq = 0;
        self.last_ping_id = None;
        self.last_ping_sent_at = None;

        Ok(())
    }

    fn send_reliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        let ws = self
            .ws
            .as_mut()
            .ok_or_else(|| NetworkError::SendFailed("not connected".to_string()))?;
        ws.send(Message::Binary(payload.to_vec().into()))
            .map_err(|e| NetworkError::SendFailed(format!("WebSocket send failed: {}", e)))
    }

    fn send_unreliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        // WebSocket is reliable by nature; delegate to send_reliable
        self.send_reliable(payload)
    }

    fn receive_reliable(&mut self) -> Vec<NetworkMessage> {
        let ws = match self.ws.as_mut() {
            Some(ws) => ws,
            None => return vec![],
        };

        let now = Instant::now();

        // Send heartbeat ping if interval elapsed
        if now.duration_since(self.last_ping_at) >= self.config.heartbeat_interval {
            self.ping_seq = self.ping_seq.wrapping_add(1);
            let payload = self.ping_seq.to_be_bytes().to_vec();
            if ws.send(Message::Ping(payload)).is_err() {
                tracing::warn!("WebSocketTransport: heartbeat ping failed, disconnecting");
                self.ws = None;
                return vec![];
            }
            self.last_ping_at = now;
            self.last_ping_id = Some(self.ping_seq);
            self.last_ping_sent_at = Some(now);
        }

        // Check heartbeat timeout
        if now.duration_since(self.last_activity_at) >= self.config.heartbeat_timeout {
            tracing::warn!("WebSocketTransport: heartbeat timeout, disconnecting");
            self.ws = None;
            return vec![];
        }

        let mut messages = Vec::new();
        let source = NetworkAddress::Custom {
            scheme: if self.config.use_tls { "wss".to_string() } else { "ws".to_string() },
            address: format!("{}:{}", self.config.host, self.config.port),
        };

        loop {
            match ws.read() {
                Ok(Message::Binary(data)) => {
                    self.last_activity_at = Instant::now();
                    messages.push(NetworkMessage {
                        source: source.clone(),
                        payload: data.into(),
                        timestamp: Instant::now(),
                    });
                }
                Ok(Message::Ping(payload)) => {
                    self.last_activity_at = Instant::now();
                    if ws.send(Message::Pong(payload)).is_err() {
                        tracing::warn!("WebSocketTransport: pong send failed, disconnecting");
                        self.ws = None;
                        break;
                    }
                }
                Ok(Message::Pong(payload)) => {
                    let rx_at = Instant::now();
                    self.last_activity_at = rx_at;
                    if payload.len() == 8 {
                        let mut buf = [0u8; 8];
                        buf.copy_from_slice(&payload[..8]);
                        let pong_id = u64::from_be_bytes(buf);
                        if Some(pong_id) == self.last_ping_id {
                            if let Some(sent_at) = self.last_ping_sent_at {
                                let rtt = rx_at.duration_since(sent_at);
                                tracing::trace!("WebSocketTransport: ping rtt_ms={:.1}", rtt.as_secs_f64() * 1000.0);
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("WebSocketTransport: server sent close");
                    self.ws = None;
                    break;
                }
                Ok(_unsupported) => {
                    // Text or other — unexpected
                    tracing::warn!("WebSocketTransport: unexpected WebSocket message type");
                }
                Err(tungstenite::Error::Io(ref e))
                    if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // No more data available — normal for non-blocking
                    break;
                }
                Err(tungstenite::Error::ConnectionClosed) => {
                    tracing::info!("WebSocketTransport: connection closed by server");
                    self.ws = None;
                    break;
                }
                Err(e) => {
                    tracing::warn!("WebSocketTransport: read error: {}", e);
                    self.ws = None;
                    break;
                }
            }
        }

        messages
    }

    fn receive_unreliable(&mut self) -> Vec<NetworkMessage> {
        // WebSocket has no unreliable channel; delegate to reliable
        self.receive_reliable()
    }

    fn wait_for_response_reliable(&mut self) -> Result<NetworkMessage, NetworkError> {
        let timeout = Duration::from_secs(10);
        let start = Instant::now();
        loop {
            let msgs = self.receive_reliable();
            if let Some(msg) = msgs.into_iter().next() {
                return Ok(msg);
            }
            if !self.is_connected() {
                return Err(NetworkError::ConnectionFailed(
                    "disconnected while waiting for response".to_string(),
                ));
            }
            if start.elapsed() >= timeout {
                return Err(NetworkError::Timeout);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn disconnect(&mut self) {
        if let Some(ref mut ws) = self.ws {
            let _ = ws.close(None);
        }
        self.ws = None;
    }

    fn is_connected(&self) -> bool {
        self.ws.is_some()
    }
}
