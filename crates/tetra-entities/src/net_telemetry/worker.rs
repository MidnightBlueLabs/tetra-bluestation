//! Telemetry worker thread — receives events from the core and forwards them
//! over a pluggable network transport.
//!
//! Follows the same pattern as [`crate::brew::worker::BrewWorker`]:
//! - generic over any [`NetworkTransport`] implementation
//! - owns the transport
//! - reconnects on failure
//! - drives the transport's heartbeat via periodic `receive_reliable()` calls
//! - processes items from a channel in a loop

use std::time::{Duration, Instant};

use crate::{
    net_telemetry::{
        channel::{RecvEvent, TelemetrySource},
        codec::TelemetryCodecJson,
        events::TelemetryEvent,
    },
    network::transports::NetworkTransport,
};

/// How long to block waiting for a telemetry event before running a
/// maintenance cycle (heartbeat, reconnect check, etc.).
const POLL_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait between reconnection attempts when the transport is down.
const RECONNECT_DELAY: Duration = Duration::from_secs(15);

pub struct TelemetryWorker<T: NetworkTransport> {
    source: TelemetrySource,
    transport: T,
    connected: bool,
    last_connect_attempt: Option<Instant>,
}

impl<T: NetworkTransport> TelemetryWorker<T> {
    pub fn new(source: TelemetrySource, transport: T) -> Self {
        Self {
            source,
            transport,
            connected: false,
            last_connect_attempt: None,
        }
    }

    pub fn run(&mut self) {
        tracing::debug!("Telemetry worker started");
        self.try_connect();

        loop {
            // Block for up to POLL_TIMEOUT waiting for an event.
            // On timeout we still run maintenance (heartbeat / reconnect).
            match self.source.recv_timeout(POLL_TIMEOUT) {
                RecvEvent::Event(event) => {
                    tracing::debug!("telemetry event received: {:?}", event);
                    self.forward_event(&event);
                }
                RecvEvent::Timeout => {
                    // No event — fall through to heartbeat maintenance
                }
                RecvEvent::Closed => {
                    tracing::debug!("Telemetry worker: all sinks dropped, shutting down");
                    break;
                }
            }

            // Drive transport heartbeat: receive_reliable() sends pings and checks timeouts
            self.drive_heartbeat();

            // Detect fresh disconnection
            if !self.transport.is_connected() && self.connected {
                tracing::warn!("Telemetry transport disconnected");
                self.connected = false;
            }

            // Periodically retry connection when disconnected
            if !self.connected {
                let should_retry = match self.last_connect_attempt {
                    Some(last) => last.elapsed() >= RECONNECT_DELAY,
                    None => true,
                };
                if should_retry {
                    self.try_connect();
                }
            }
        }

        self.transport.disconnect();
        tracing::info!("Telemetry worker exiting");
    }

    fn forward_event(&mut self, event: &TelemetryEvent) {
        if !self.connected {
            self.try_connect();
            if !self.connected {
                return;
            }
        }

        let codec = TelemetryCodecJson;
        let payload = codec.encode(event);
        if let Err(e) = self.transport.send_reliable(&payload) {
            tracing::warn!("Telemetry transport send failed: {}, will reconnect", e);
            self.connected = false;
            self.try_connect();
        }
    }

    /// Call `receive_reliable()` on the transport to drive its internal
    /// heartbeat machinery (ping/pong, timeout detection).
    /// Any unexpected inbound messages are logged and discarded.
    fn drive_heartbeat(&mut self) {
        if !self.connected {
            return;
        }

        let msgs = self.transport.receive_reliable();
        for msg in msgs {
            tracing::trace!(
                "Telemetry: unexpected inbound message ({} bytes) from {:?}",
                msg.payload.len(),
                msg.source
            );
        }

        // After receive_reliable, transport may have detected a timeout.
        // Note: we only log here; the run() loop handles the state transition
        // and reconnection to avoid conflicting with its own detection logic.
        if !self.transport.is_connected() {
            tracing::warn!("Telemetry transport heartbeat timeout detected");
        }
    }

    fn try_connect(&mut self) {
        self.last_connect_attempt = Some(Instant::now());
        match self.transport.connect() {
            Ok(()) => {
                tracing::info!("Telemetry transport connected");
                self.connected = true;
            }
            Err(e) => {
                tracing::warn!("Telemetry transport connection failed: {}, will retry in {:?}", e, RECONNECT_DELAY);
                self.connected = false;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tetra_core::debug::setup_logging_verbose;

    use super::*;
    use crate::net_telemetry::channel::telemetry_channel;
    use crate::network::transports::NetworkAddress;
    use crate::network::transports::mock::MockTransport;
    use crate::network::transports::quic::QuicTransport;
    use crate::network::transports::websocket::{WebSocketTransport, WebSocketTransportConfig};

    #[test]
    fn test_worker_forwards_events_and_exits() {
        setup_logging_verbose();
        let (sink, source) = telemetry_channel();

        let handle = std::thread::spawn(move || {
            let mut worker = TelemetryWorker::new(source, MockTransport::new());
            worker.run();
        });

        sink.send(TelemetryEvent::MsRegistration { issi: 1234 });

        sink.send(TelemetryEvent::MsDeregistration { issi: 1234 });

        drop(sink);
        handle.join().expect("telemetry worker panicked");
    }

    /// Integration test: spins up a TelemetryWorker with a real WebSocketTransport
    /// connected to a locally running `bluestation-telemetry` on `ws://127.0.0.1:9001`.
    /// Mirrors `test_worker_forwards_events_and_exits` but goes over the network.
    ///
    /// Run with: `cargo test -p tetra-entities -- --ignored test_websocket_to_telemetry_endpoint`
    #[test]
    #[ignore] // Not run by default as it requires a running local listener
    fn test_websocket_to_telemetry_endpoint() {
        setup_logging_verbose();

        let config = WebSocketTransportConfig {
            host: "127.0.0.1".to_string(),
            port: 9001,
            use_tls: false,
            digest_auth_credentials: None,
            basic_auth_credentials: None,
            endpoint_path: "/".to_string(),
            subprotocol: None,
            user_agent: "bluestation-test".to_string(),
            heartbeat_interval: Duration::from_secs(10),
            heartbeat_timeout: Duration::from_secs(30),
            custom_root_certs: None,
        };

        let (sink, source) = telemetry_channel();

        let handle = std::thread::spawn(move || {
            let transport = WebSocketTransport::new(config);
            let mut worker = TelemetryWorker::new(source, transport);
            worker.run();
        });

        sink.send(TelemetryEvent::MsRegistration { issi: 1234 });

        sink.send(TelemetryEvent::MsDeregistration { issi: 1234 });

        drop(sink);
        handle.join().expect("telemetry worker panicked");
    }

    /// Integration test: spins up a TelemetryWorker with a real QuicTransport
    /// connected to a locally running `bluestation-telemetry-quic` on `127.0.0.1:4434`.
    /// Mirrors `test_worker_forwards_events_and_exits` but goes over QUIC.
    ///
    /// Run with: `cargo test -p tetra-entities -- --ignored test_quic_to_telemetry_endpoint`
    #[test]
    #[ignore] // Not run by default as it requires a running local QUIC listener
    fn test_quic_to_telemetry_endpoint() {
        setup_logging_verbose();

        let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");

        let server_addr = NetworkAddress::Udp {
            host: "127.0.0.1".to_string(),
            port: 4434,
        };

        let transport = QuicTransport::new(
            server_addr,
            Duration::from_secs(5),
            true, // skip cert verification for self-signed test cert
            runtime,
        )
        .expect("failed to create QUIC transport");

        let (sink, source) = telemetry_channel();

        let handle = std::thread::spawn(move || {
            let mut worker = TelemetryWorker::new(source, transport);
            worker.run();
        });

        sink.send(TelemetryEvent::MsRegistration { issi: 1234 });

        sink.send(TelemetryEvent::MsDeregistration { issi: 1234 });

        drop(sink);
        handle.join().expect("telemetry worker panicked");
    }

    /// Integration test: connect to localhost:19001 with correct Basic Auth credentials.
    ///
    /// Requires a running telemetry service:
    /// ```sh
    /// printf 'testuser:testpass\n' > /tmp/test_authfile.txt
    /// ./target/debug/bluestation-telemetry-service --listen 127.0.0.1:19001 --auth-file /tmp/test_authfile.txt
    /// ```
    ///
    /// Run: `cargo test -p tetra-entities -- --ignored test_basic_auth_accepted`
    #[test]
    #[ignore]
    fn test_basic_auth_accepted() {
        setup_logging_verbose();

        let config = WebSocketTransportConfig {
            host: "127.0.0.1".to_string(),
            port: 19001,
            use_tls: false,
            digest_auth_credentials: None,
            basic_auth_credentials: Some(("testuser".to_string(), "testpass".to_string())),
            endpoint_path: "/".to_string(),
            subprotocol: Some(crate::net_telemetry::TELEMETRY_PROTOCOL_VERSION.to_string()),
            user_agent: "bluestation-test".to_string(),
            heartbeat_interval: Duration::from_secs(10),
            heartbeat_timeout: Duration::from_secs(30),
            custom_root_certs: None,
        };

        let mut transport = WebSocketTransport::new(config);
        let result = transport.connect();
        assert!(result.is_ok(), "connect with valid Basic Auth should succeed: {:?}", result);
        assert!(transport.is_connected());
    }

    /// Integration test: connect to localhost:19001 with wrong credentials (should be rejected).
    ///
    /// Run: `cargo test -p tetra-entities -- --ignored test_basic_auth_rejected`
    #[test]
    #[ignore]
    fn test_basic_auth_rejected() {
        setup_logging_verbose();

        let config = WebSocketTransportConfig {
            host: "127.0.0.1".to_string(),
            port: 19001,
            use_tls: false,
            digest_auth_credentials: None,
            basic_auth_credentials: Some(("wrong".to_string(), "creds".to_string())),
            endpoint_path: "/".to_string(),
            subprotocol: Some(crate::net_telemetry::TELEMETRY_PROTOCOL_VERSION.to_string()),
            user_agent: "bluestation-test".to_string(),
            heartbeat_interval: Duration::from_secs(10),
            heartbeat_timeout: Duration::from_secs(30),
            custom_root_certs: None,
        };

        let mut transport = WebSocketTransport::new(config);
        let result = transport.connect();
        assert!(result.is_err(), "connect with wrong credentials should fail");
    }

    /// Integration test: connect without credentials to an auth-required endpoint (should fail).
    ///
    /// Run: `cargo test -p tetra-entities -- --ignored test_basic_auth_missing`
    #[test]
    #[ignore]
    fn test_basic_auth_missing() {
        setup_logging_verbose();

        let config = WebSocketTransportConfig {
            host: "127.0.0.1".to_string(),
            port: 19001,
            use_tls: false,
            digest_auth_credentials: None,
            basic_auth_credentials: None,
            endpoint_path: "/".to_string(),
            subprotocol: Some(crate::net_telemetry::TELEMETRY_PROTOCOL_VERSION.to_string()),
            user_agent: "bluestation-test".to_string(),
            heartbeat_interval: Duration::from_secs(10),
            heartbeat_timeout: Duration::from_secs(30),
            custom_root_certs: None,
        };

        let mut transport = WebSocketTransport::new(config);
        let result = transport.connect();
        assert!(result.is_err(), "connect without credentials to auth endpoint should fail");
    }
}
