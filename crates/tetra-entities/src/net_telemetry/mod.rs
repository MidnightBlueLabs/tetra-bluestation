//! External networked telemetry component
//!
//! Runs outside the real-time core in its own thread. Receives [`TelemetryEvent`]s
//! from the core via a [`TelemetrySource`] and forwards them over a pluggable
//! network transport.

pub mod channel;
pub mod codec;
pub mod events;
pub mod worker;

use std::time::Duration;

pub use self::channel::{TelemetrySink, TelemetrySource, telemetry_channel};
pub use self::events::TelemetryEvent;
pub use self::worker::TelemetryWorker;

/// Sent as subprotocol in WebSocket handshake
pub const TELEMETRY_PROTOCOL_VERSION: &str = "bluestation-telemetry-v1";
pub const TELEMETRY_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
pub const TELEMETRY_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
