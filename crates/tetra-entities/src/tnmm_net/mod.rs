pub mod codec;
pub mod test_pdu;

pub mod net_entity_tnmm_worker;

// Re-export convenience type aliases for common transport configurations
use crate::network::transports::quic::QuicTransport;
use crate::network::transports::tcp::TcpTransport;
use net_entity_tnmm_worker::NetEntityTnmmWorker;

/// TCP-based TNMM worker
pub type TnmmWorkerTcp = NetEntityTnmmWorker<TcpTransport>;

/// QUIC-based TNMM worker
pub type TnmmWorkerQuic = NetEntityTnmmWorker<QuicTransport>;
