use std::net::UdpSocket;
use std::time::{Duration, Instant};

use super::{NetworkAddress, NetworkError, NetworkMessage, NetworkTransport};

/// UDP-based network transport
pub struct UdpTransport {
    socket: Option<UdpSocket>,
    server_addr: NetworkAddress,
    bind_addr: String,
}

impl UdpTransport {
    pub fn new(server_addr: NetworkAddress, bind_addr: String) -> Self {
        Self {
            socket: None,
            server_addr,
            bind_addr,
        }
    }

    fn ensure_connected(&mut self) -> Result<(), NetworkError> {
        if self.socket.is_none() {
            self.connect()?;
        }
        Ok(())
    }

    fn get_udp_addr(&self) -> Result<String, NetworkError> {
        match &self.server_addr {
            NetworkAddress::Udp { host, port } => Ok(format!("{}:{}", host, port)),
            _ => Err(NetworkError::ConnectionFailed("Invalid address type for UDP transport".to_string())),
        }
    }
}

impl NetworkTransport for UdpTransport {
    fn send_unreliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        self.ensure_connected()?;

        if let Some(ref socket) = self.socket {
            let addr = self.get_udp_addr()?;

            socket
                .send_to(payload, &addr)
                .map_err(|e| NetworkError::SendFailed(format!("UDP send failed: {}", e)))?;

            Ok(())
        } else {
            Err(NetworkError::SendFailed("No active socket".to_string()))
        }
    }

    fn receive_unreliable(&mut self) -> Vec<NetworkMessage> {
        let mut messages = Vec::new();

        if let Some(ref socket) = self.socket {
            // Set non-blocking mode
            if let Err(_) = socket.set_nonblocking(true) {
                return messages;
            }

            loop {
                let mut buffer = vec![0u8; 65536]; // Max UDP packet size
                match socket.recv_from(&mut buffer) {
                    Ok((len, addr)) => {
                        buffer.truncate(len);

                        // Convert SocketAddr to NetworkAddress
                        let source = NetworkAddress::Udp {
                            host: addr.ip().to_string(),
                            port: addr.port(),
                        };

                        messages.push(NetworkMessage {
                            source,
                            payload: buffer,
                            timestamp: Instant::now(),
                        });
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        break; // No more data
                    }
                    Err(_) => break, // Error
                }
            }

            // Restore blocking mode
            let _ = socket.set_nonblocking(false);
        }

        messages
    }

    fn connect(&mut self) -> Result<(), NetworkError> {
        match UdpSocket::bind(&self.bind_addr) {
            Ok(socket) => {
                // Set reasonable timeout
                socket
                    .set_read_timeout(Some(Duration::from_millis(100)))
                    .map_err(|e| NetworkError::ConnectionFailed(format!("Failed to set timeout: {}", e)))?;

                self.socket = Some(socket);
                Ok(())
            }
            Err(e) => Err(NetworkError::ConnectionFailed(format!("UDP bind failed: {}", e))),
        }
    }

    fn send_reliable(&mut self, _payload: &[u8]) -> Result<(), NetworkError> {
        unimplemented!();
    }

    fn receive_reliable(&mut self) -> Vec<NetworkMessage> {
        unimplemented!();
    }

    fn wait_for_response_reliable(&mut self) -> Result<NetworkMessage, NetworkError> {
        unimplemented!();
    }
}
