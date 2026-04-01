use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use super::{NetworkAddress, NetworkError, NetworkMessage, NetworkTransport, TransportFactory};

/// Configuration for creating a UDP transport.
#[derive(Debug, Clone)]
pub struct UdpTransportConfig {
    /// Remote UDP endpoint used for send/receive.
    pub remote_addr: NetworkAddress,
    /// Optional local UDP bind endpoint. If omitted, an ephemeral port is used.
    pub bind_addr: Option<NetworkAddress>,
    /// Read timeout used by blocking receive calls.
    pub read_timeout: Duration,
}

impl UdpTransportConfig {
    /// Create a new UDP transport configuration with a default read timeout.
    pub fn new(remote_addr: NetworkAddress) -> Self {
        Self {
            remote_addr,
            bind_addr: None,
            read_timeout: Duration::from_secs(1),
        }
    }

    /// Bind the socket to a specific local endpoint.
    pub fn with_bind_addr(mut self, bind_addr: NetworkAddress) -> Self {
        self.bind_addr = Some(bind_addr);
        self
    }

    /// Override the blocking read timeout.
    pub fn with_read_timeout(mut self, read_timeout: Duration) -> Self {
        self.read_timeout = read_timeout;
        self
    }
}

/// UDP-based network transport.
pub struct UdpTransport {
    socket: Option<UdpSocket>,
    remote_addr: NetworkAddress,
    bind_addr: Option<NetworkAddress>,
    read_timeout: Duration,
}

impl UdpTransport {
    pub fn new(config: UdpTransportConfig) -> Self {
        Self {
            socket: None,
            remote_addr: config.remote_addr,
            bind_addr: config.bind_addr,
            read_timeout: config.read_timeout,
        }
    }

    fn ensure_socket_exists(&mut self) -> Result<(), NetworkError> {
        if self.socket.is_none() {
            self.connect()?;
        }
        Ok(())
    }

    fn close_socket(&mut self) {
        self.socket = None;
    }

    fn resolve_udp_addr(addr: &NetworkAddress) -> Result<SocketAddr, NetworkError> {
        let NetworkAddress::Udp { host, port } = addr else {
            return Err(NetworkError::ConnectionFailed("Invalid address type for UdpTransport".to_string()));
        };

        (host.as_str(), *port)
            .to_socket_addrs()
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to resolve UDP address {host}:{port}: {e}")))?
            .next()
            .ok_or_else(|| NetworkError::ConnectionFailed(format!("No UDP addresses resolved for {host}:{port}")))
    }

    fn default_bind_addr(remote_addr: &SocketAddr) -> SocketAddr {
        if remote_addr.is_ipv4() {
            "0.0.0.0:0".parse().expect("hardcoded IPv4 bind addr must parse")
        } else {
            "[::]:0".parse().expect("hardcoded IPv6 bind addr must parse")
        }
    }

    fn receive_pending(&mut self) -> Vec<NetworkMessage> {
        let mut messages = Vec::new();
        let mut drop_socket = false;

        if let Some(ref mut socket) = self.socket {
            if let Err(e) = socket.set_nonblocking(true) {
                tracing::warn!("UdpTransport: failed to enable non-blocking mode: {}", e);
                return messages;
            }

            loop {
                let mut payload = vec![0u8; 65535];
                match socket.recv_from(&mut payload) {
                    Ok((len, source)) => {
                        payload.truncate(len);
                        messages.push(NetworkMessage {
                            source: NetworkAddress::Udp {
                                host: source.ip().to_string(),
                                port: source.port(),
                            },
                            payload,
                            timestamp: Instant::now(),
                        });
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("UdpTransport receive failed: {}", e);
                        drop_socket = true;
                        break;
                    }
                }
            }

            let _ = socket.set_nonblocking(false);
        }

        if drop_socket {
            self.close_socket();
        }

        messages
    }
}

impl TransportFactory for UdpTransport {
    type Config = UdpTransportConfig;

    fn create(config: Self::Config) -> Result<Self, NetworkError> {
        Ok(Self::new(config))
    }
}

impl NetworkTransport for UdpTransport {
    fn connect(&mut self) -> Result<(), NetworkError> {
        self.close_socket();

        let remote_addr = Self::resolve_udp_addr(&self.remote_addr)?;
        let bind_addr = match &self.bind_addr {
            Some(bind_addr) => Self::resolve_udp_addr(bind_addr)?,
            None => Self::default_bind_addr(&remote_addr),
        };

        let socket =
            UdpSocket::bind(bind_addr).map_err(|e| NetworkError::ConnectionFailed(format!("UDP bind to {bind_addr} failed: {e}")))?;
        socket
            .connect(remote_addr)
            .map_err(|e| NetworkError::ConnectionFailed(format!("UDP connect to {remote_addr} failed: {e}")))?;
        socket
            .set_read_timeout(Some(self.read_timeout))
            .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to set UDP read timeout: {e}")))?;

        self.socket = Some(socket);
        Ok(())
    }

    fn send_reliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        self.send_unreliable(payload)
    }

    fn send_unreliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        self.ensure_socket_exists()?;
        let send_result = {
            let Some(ref socket) = self.socket else {
                return Err(NetworkError::SendFailed("UDP socket not connected".to_string()));
            };
            socket.send(payload)
        };

        match send_result {
            Ok(_) => Ok(()),
            Err(e) => {
                self.close_socket();
                Err(NetworkError::SendFailed(format!("UDP send failed: {e}")))
            }
        }
    }

    fn receive_reliable(&mut self) -> Vec<NetworkMessage> {
        self.receive_pending()
    }

    fn receive_unreliable(&mut self) -> Vec<NetworkMessage> {
        self.receive_pending()
    }

    fn wait_for_response_reliable(&mut self) -> Result<NetworkMessage, NetworkError> {
        self.ensure_socket_exists()?;

        let recv_result = {
            let Some(ref mut socket) = self.socket else {
                return Err(NetworkError::ReceiveFailed("UDP socket not connected".to_string()));
            };

            let mut payload = vec![0u8; 65535];
            match socket.recv_from(&mut payload) {
                Ok((len, source)) => Ok((payload, len, source)),
                Err(e) => Err(e),
            }
        };

        match recv_result {
            Ok((mut payload, len, source)) => {
                payload.truncate(len);
                Ok(NetworkMessage {
                    source: NetworkAddress::Udp {
                        host: source.ip().to_string(),
                        port: source.port(),
                    },
                    payload,
                    timestamp: Instant::now(),
                })
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => Err(NetworkError::Timeout),
            Err(e) => {
                let err = NetworkError::ReceiveFailed(format!("UDP receive failed: {e}"));
                self.close_socket();
                Err(err)
            }
        }
    }

    fn disconnect(&mut self) {
        self.close_socket();
    }

    fn is_connected(&self) -> bool {
        self.socket.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_udp_transport_sends_datagram() {
        let listener = UdpSocket::bind("127.0.0.1:0").expect("failed to bind listener");
        listener
            .set_read_timeout(Some(Duration::from_secs(1)))
            .expect("failed to set listener timeout");
        let listener_addr = listener.local_addr().expect("failed to read listener addr");

        let mut transport = UdpTransport::new(UdpTransportConfig::new(NetworkAddress::Udp {
            host: "127.0.0.1".to_string(),
            port: listener_addr.port(),
        }));

        transport.connect().expect("udp connect failed");
        transport.send_unreliable(b"hello").expect("udp send failed");

        let mut buf = [0u8; 16];
        let (len, _) = listener.recv_from(&mut buf).expect("listener recv failed");
        assert_eq!(&buf[..len], b"hello");
    }
}
