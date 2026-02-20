use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use quinn::{Connection, Endpoint, RecvStream, SendStream, VarInt};
use rustls::pki_types::{CertificateDer, ServerName};

use super::{NetworkAddress, NetworkError, NetworkMessage, NetworkTransport};

/// Channel type for QUIC streams
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuicChannelType {
    /// Reliable ordered channel for signalling
    Reliable,
    /// Unreliable unordered channel for voice (low latency)
    Unreliable,
}

/// Configuration for creating a QUIC transport
#[derive(Clone)]
pub struct QuicTransportConfig {
    /// Server address to connect to
    pub server_addr: NetworkAddress,
    /// Connection timeout
    pub connect_timeout: Duration,
    /// Skip certificate verification (testing only)
    pub skip_cert_verification: bool,
    /// Tokio runtime handle for async operations
    pub runtime: tokio::runtime::Handle,
}

impl QuicTransportConfig {
    /// Create a new QUIC transport configuration
    pub fn new(server_addr: NetworkAddress, runtime: tokio::runtime::Handle) -> Self {
        Self {
            server_addr,
            connect_timeout: Duration::from_secs(5),
            skip_cert_verification: false,
            runtime,
        }
    }

    /// Create an insecure QUIC transport configuration (for testing)
    pub fn insecure(server_addr: NetworkAddress, runtime: tokio::runtime::Handle) -> Self {
        Self {
            server_addr,
            connect_timeout: Duration::from_secs(5),
            skip_cert_verification: true,
            runtime,
        }
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }
}

/// QUIC-based network transport
/// Provides both reliable signalling and unreliable voice channels
pub struct QuicTransport {
    endpoint: Option<Endpoint>,
    connection: Option<Connection>,
    server_addr: NetworkAddress,
    socket_addr: SocketAddr,
    connect_timeout: Duration,
    /// Cached reliable bi-directional stream for signalling
    reliable_send: Option<SendStream>,
    reliable_recv: Option<RecvStream>,
    /// Tokio runtime handle for async operations
    runtime: tokio::runtime::Runtime,
}

impl QuicTransport {
    /// Create a new QUIC transport
    ///
    /// # Arguments
    /// * `server_addr` - Server address to connect to
    /// * `connect_timeout` - Connection timeout duration
    /// * `skip_cert_verification` - Skip certificate verification (useful for testing)
    /// * `runtime` - Tokio runtimefor async operations
    pub fn new(
        server_addr: NetworkAddress,
        connect_timeout: Duration,
        skip_cert_verification: bool,
        runtime: tokio::runtime::Runtime,
    ) -> Result<Self, NetworkError> {
        let socket_addr = Self::parse_socket_addr(&server_addr)?;

        // Create endpoint within the runtime context
        let endpoint = runtime.block_on(async {
            // Configure QUIC client
            let crypto = if skip_cert_verification {
                Self::configure_insecure_client()
            } else {
                Self::configure_client()
            }?;

            let mut client_config = quinn::ClientConfig::new(Arc::new(crypto));

            // Configure transport for low latency
            let mut transport_config = quinn::TransportConfig::default();

            // Reduce keep-alive interval for faster detection of disconnects
            transport_config.keep_alive_interval(Some(Duration::from_secs(5)));

            // Lower max idle timeout
            transport_config.max_idle_timeout(Some(VarInt::from_u32(30_000).into()));

            client_config.transport_config(Arc::new(transport_config));

            // Create endpoint
            let mut endpoint = Endpoint::client("[::]:0".parse().unwrap())
                .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to create endpoint: {}", e)))?;

            endpoint.set_default_client_config(client_config);

            Ok::<_, NetworkError>(endpoint)
        })?;

        Ok(Self {
            endpoint: Some(endpoint),
            connection: None,
            server_addr,
            socket_addr,
            connect_timeout,
            reliable_send: None,
            reliable_recv: None,
            runtime,
        })
    }

    /// Parse NetworkAddress to SocketAddr
    fn parse_socket_addr(addr: &NetworkAddress) -> Result<SocketAddr, NetworkError> {
        match addr {
            NetworkAddress::Tcp { host, port } | NetworkAddress::Udp { host, port } => format!("{}:{}", host, port)
                .parse()
                .map_err(|e| NetworkError::ConnectionFailed(format!("Invalid address: {}", e))),
            NetworkAddress::Custom { .. } => Err(NetworkError::ConnectionFailed(
                "Custom addresses not supported for QUIC".to_string(),
            )),
        }
    }

    /// Configure client with certificate verification (production)
    fn configure_client() -> Result<quinn::crypto::rustls::QuicClientConfig, NetworkError> {
        let mut roots = rustls::RootCertStore::empty();

        // Add system certificates
        for cert in rustls_native_certs::load_native_certs()
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to load native certs: {}", e)))?
        {
            roots
                .add(cert)
                .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to add cert: {}", e)))?;
        }

        let mut crypto = rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();

        // Set ALPN protocol for QUIC
        crypto.alpn_protocols = vec![b"hq-29".to_vec()];

        Ok(quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to build QUIC config: {}", e)))?)
    }

    /// Configure insecure client (skip certificate verification - testing only)
    fn configure_insecure_client() -> Result<quinn::crypto::rustls::QuicClientConfig, NetworkError> {
        let mut crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        // Set ALPN protocol for QUIC
        crypto.alpn_protocols = vec![b"hq-29".to_vec()];

        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to build insecure QUIC config: {}", e)))
    }

    /// Send data on a specific channel type
    pub async fn send_on_channel(&mut self, payload: &[u8], channel: QuicChannelType) -> Result<(), NetworkError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| NetworkError::SendFailed("No active connection".to_string()))?;

        match channel {
            QuicChannelType::Reliable => {
                // Use cached bidirectional stream for signalling
                if self.reliable_send.is_none() {
                    let (send, recv) = conn
                        .open_bi()
                        .await
                        .map_err(|e| NetworkError::SendFailed(format!("Failed to open stream: {}", e)))?;
                    self.reliable_send = Some(send);
                    self.reliable_recv = Some(recv);
                }

                let send = self.reliable_send.as_mut().unwrap();

                // Send length-prefixed message
                let len = (payload.len() as u32).to_be_bytes();
                send.write_all(&len)
                    .await
                    .map_err(|e| NetworkError::SendFailed(format!("Failed to send length: {}", e)))?;
                send.write_all(payload)
                    .await
                    .map_err(|e| NetworkError::SendFailed(format!("Failed to send payload: {}", e)))?;
            }
            QuicChannelType::Unreliable => {
                // Send as unreliable datagram for low latency voice
                conn.send_datagram(payload.to_vec().into())
                    .map_err(|e| NetworkError::SendFailed(format!("Failed to send datagram: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Receive data from a specific channel type (non-blocking)
    pub async fn receive_from_channel(&mut self, channel: QuicChannelType) -> Result<Option<Vec<u8>>, NetworkError> {
        let conn = self
            .connection
            .as_ref()
            .ok_or_else(|| NetworkError::ReceiveFailed("No active connection".to_string()))?;

        match channel {
            QuicChannelType::Reliable => {
                if let Some(ref mut recv) = self.reliable_recv {
                    // Try to read length prefix
                    let mut len_buf = [0u8; 4];
                    match recv.read_exact(&mut len_buf).await {
                        Ok(()) => {
                            let len = u32::from_be_bytes(len_buf) as usize;

                            if len > 1024 * 1024 {
                                return Err(NetworkError::ReceiveFailed("Message too large".to_string()));
                            }

                            let mut payload = vec![0u8; len];
                            match recv.read_exact(&mut payload).await {
                                Ok(()) => Ok(Some(payload)),
                                Err(_) => Ok(None), // Stream finished
                            }
                        }
                        Err(_) => Ok(None), // Stream finished or error
                    }
                } else {
                    Ok(None)
                }
            }
            QuicChannelType::Unreliable => {
                // Receive datagram
                match conn.read_datagram().await {
                    Ok(data) => Ok(Some(data.to_vec())),
                    Err(quinn::ConnectionError::ApplicationClosed(_)) => Ok(None),
                    Err(e) => Err(NetworkError::ReceiveFailed(format!("Failed to receive datagram: {}", e))),
                }
            }
        }
    }

    /// Close reliable stream
    pub fn close_reliable_stream(&mut self) {
        if let Some(mut send) = self.reliable_send.take() {
            let _ = send.finish();
        }
        self.reliable_recv = None;
    }
}

impl NetworkTransport for QuicTransport {
    fn connect(&mut self) -> Result<(), NetworkError> {
        tracing::debug!("QuicTransport connecting to {:?}", self.server_addr);

        // Close existing connection
        if let Some(conn) = self.connection.take() {
            conn.close(VarInt::from_u32(0), b"reconnecting");
        }
        self.close_reliable_stream();

        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| NetworkError::ConnectionFailed("No endpoint".to_string()))?;

        // Extract server name for SNI
        let server_name = match &self.server_addr {
            NetworkAddress::Tcp { host, .. } | NetworkAddress::Udp { host, .. } => host.clone(),
            _ => {
                return Err(NetworkError::ConnectionFailed("Invalid address type".to_string()));
            }
        };

        // Connect with timeout
        let runtime = self.runtime.handle().clone();
        let connection = runtime.block_on(async {
            let connecting = endpoint
                .connect(self.socket_addr, &server_name)
                .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to initiate connection: {}", e)))?;

            tokio::time::timeout(self.connect_timeout, connecting)
                .await
                .map_err(|_| NetworkError::Timeout)?
                .map_err(|e| NetworkError::ConnectionFailed(format!("Connection failed: {}", e)))
        })?;

        self.connection = Some(connection);

        Ok(())
    }

    fn send_reliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        // Synchronous wrapper around async send_on_channel
        let runtime = self.runtime.handle().clone();
        runtime.block_on(async { self.send_on_channel(payload, QuicChannelType::Reliable).await })
    }

    fn send_unreliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        // Synchronous wrapper around async send_on_channel
        let runtime = self.runtime.handle().clone();
        runtime.block_on(async { self.send_on_channel(payload, QuicChannelType::Unreliable).await })
    }

    fn receive_reliable(&mut self) -> Vec<NetworkMessage> {
        // Non-blocking receive from reliable channel
        let mut messages = Vec::new();

        let runtime = self.runtime.handle().clone();
        if let Ok(Some(payload)) = runtime.block_on(async { self.receive_from_channel(QuicChannelType::Reliable).await }) {
            messages.push(NetworkMessage {
                source: self.server_addr.clone(),
                payload,
                timestamp: Instant::now(),
            });
        }

        messages
    }

    fn receive_unreliable(&mut self) -> Vec<NetworkMessage> {
        // Non-blocking receive from unreliable channel (datagrams)
        let mut messages = Vec::new();

        let runtime = self.runtime.handle().clone();
        if let Ok(Some(payload)) = runtime.block_on(async { self.receive_from_channel(QuicChannelType::Unreliable).await }) {
            messages.push(NetworkMessage {
                source: self.server_addr.clone(),
                payload,
                timestamp: Instant::now(),
            });
        }

        messages
    }

    fn wait_for_response_reliable(&mut self) -> Result<NetworkMessage, NetworkError> {
        // Blocking receive from reliable channel
        let runtime = self.runtime.handle().clone();
        runtime.block_on(async {
            match self.receive_from_channel(QuicChannelType::Reliable).await? {
                Some(payload) => Ok(NetworkMessage {
                    source: self.server_addr.clone(),
                    payload,
                    timestamp: Instant::now(),
                }),
                None => Err(NetworkError::ReceiveFailed("Connection closed".to_string())),
            }
        })
    }
}

impl Drop for QuicTransport {
    fn drop(&mut self) {
        if let Some(conn) = self.connection.take() {
            conn.close(VarInt::from_u32(0), b"shutdown");
        }
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close(VarInt::from_u32(0), b"shutdown");
        }
    }
}

/// Certificate verifier that accepts any certificate (INSECURE - testing only)
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
