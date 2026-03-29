//! External networked command component
//!
//! Runs outside the real-time core in its own thread. Receives [`Command`]s
//! from a remote server via a pluggable network transport, forwards them
//! toward the stack through a [`CommandSink`], and sends [`CommandResponse`]s
//! back to the server.

pub mod channel;
pub mod codec;
pub mod commands;
pub mod worker;

use std::time::Duration;

pub use self::channel::{CommandDispatcher, ControlEndpoint, make_control_link};
pub use self::commands::{ControlCommand, ControlResponse};
pub use self::worker::ControlWorker;

/// Sent as subprotocol in WebSocket handshake
pub const CONTROL_PROTOCOL_VERSION: &str = "bluestation-control-v1";
pub const CONTROL_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
pub const CONTROL_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
