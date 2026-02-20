use std::time::Instant;

use serde::{Deserialize, Serialize};

pub mod quic;
pub mod tcp;
pub mod udp;

/// Network transport abstraction for Entity-to-network external communications
///
/// This trait defines a unified interface for both reliable (TCP, QUIC streams)
/// and unreliable (UDP, QUIC datagrams) transports. Transports should either
/// implement those methods or raise an unimplemented!() panic.
pub trait NetworkTransport: Send {
    /// Connect or reconnect the transport. Destroys any existing connection.
    fn connect(&mut self) -> Result<(), NetworkError>;

    /// Send a message reliably (guaranteed delivery, ordered arrival)
    fn send_reliable(&mut self, payload: &[u8]) -> Result<(), NetworkError>;

    /// Send a message unreliably (no delivery guarantee, unordered, lower latency)
    fn send_unreliable(&mut self, payload: &[u8]) -> Result<(), NetworkError>;

    /// Receive pending messages from the reliable channel (non-blocking)
    fn receive_reliable(&mut self) -> Vec<NetworkMessage>;

    /// Receive pending messages from the unreliable channel (non-blocking)
    fn receive_unreliable(&mut self) -> Vec<NetworkMessage>;

    /// Wait for a single response on the reliable channel (blocking with timeout)
    fn wait_for_response_reliable(&mut self) -> Result<NetworkMessage, NetworkError>;
}

/// Factory trait for creating transport instances
///
/// Each transport type implements this to define how it gets constructed
/// from a configuration type. This allows generic workers to create transports
/// without knowing the specific construction details.
pub trait TransportFactory: NetworkTransport + Sized {
    /// Configuration type needed to construct this transport
    type Config: Send + 'static;

    /// Create a new transport instance from configuration
    fn create(config: Self::Config) -> Result<Self, NetworkError>;
}

/// Network address abstraction
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkAddress {
    /// TCP endpoint
    Tcp { host: String, port: u16 },
    /// UDP endpoint  
    Udp { host: String, port: u16 },
    /// Custom addressing scheme
    Custom { scheme: String, address: String },
}

/// Network message received from external source
#[derive(Debug, Clone)]
pub struct NetworkMessage {
    pub source: NetworkAddress,
    pub payload: Vec<u8>,
    pub timestamp: Instant,
}

/// Network-related errors
#[derive(Debug, Clone)]
pub enum NetworkError {
    ConnectionFailed(String),
    SendFailed(String),
    ReceiveFailed(String),
    SerializationError(String),
    InvalidService(String),
    InvalidServiceVersion(String),
    Timeout,
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            NetworkError::SendFailed(msg) => write!(f, "Send failed: {}", msg),
            NetworkError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            NetworkError::InvalidService(msg) => write!(f, "Invalid service: {}", msg),
            NetworkError::InvalidServiceVersion(msg) => {
                write!(f, "Invalid service version: {}", msg)
            }
            NetworkError::ReceiveFailed(_) => write!(f, "Receive failed"),
            NetworkError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl std::error::Error for NetworkError {}
