//! Brew protocol integration for TETRA group call bridging via a pluggable network transport
//!
//! The transport (WebSocket, QUIC, TCP, …) is injected at construction time.
//! See [`websocket_transport_config`] for the default WebSocket configuration
//! used with TetraPack/BrandMeister.

pub mod components;
pub mod entity;
pub mod protocol;
pub mod worker;

pub use components::brew_routable::feature_sds_enabled;
/// Convenience re-export of commonly externally used functions
pub use components::brew_routable::is_active;
pub use components::brew_routable::is_brew_gssi_routable;
pub use components::brew_routable::is_brew_issi_routable;

use std::time::Duration;

use crate::network::transports::websocket::{WebSocketTransport, WebSocketTransportConfig};
use tetra_config::bluestation::CfgBrew;

pub const BREW_PROTOCOL_VERSION: &str = "brew";

/// Build a [`WebSocketTransportConfig`] from the Brew section of the stack config.
///
/// This wires the Brew-specific defaults (endpoint path `/brew/`, subprotocol `"brew"`,
/// heartbeat intervals) into the generic WebSocket transport.
pub fn websocket_transport_config(cfg: &CfgBrew) -> WebSocketTransportConfig {
    WebSocketTransportConfig {
        host: cfg.host.clone(),
        port: cfg.port,
        use_tls: cfg.tls,
        digest_auth_credentials: match (&cfg.username, &cfg.password) {
            (Some(u), Some(p)) => Some((u.clone(), p.clone())),
            _ => None,
        },
        endpoint_path: "/brew/".to_string(),
        subprotocol: Some(BREW_PROTOCOL_VERSION.to_string()),
        user_agent: format!("BlueStation/{}", tetra_core::STACK_VERSION),
        heartbeat_interval: Duration::from_secs(10),
        heartbeat_timeout: Duration::from_secs(30),
        custom_root_certs: None,
        basic_auth_credentials: None,
    }
}

/// Create a [`WebSocketTransport`] configured for Brew from the stack config.
pub fn new_websocket_transport(cfg: &CfgBrew) -> WebSocketTransport {
    WebSocketTransport::new(websocket_transport_config(cfg))
}
