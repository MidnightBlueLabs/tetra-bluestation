//! Command worker thread — receives commands from a remote server via a
//! pluggable network transport, dispatches them to the appropriate entity
//! through per-entity [`CommandDispatcher`] links, collects
//! [`CommandResponse`]s, and sends them back over the network.
//!
//! - decodes inbound messages as [`Command`]s, interprets how command must be handled, dispatches via per-entity links
//! - collects [`CommandResponse`]s from entities and sends them back

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tetra_core::tetra_entities::TetraEntity;

use crate::{
    net_control::{
        channel::CommandDispatcher,
        codec::ControlCodecJson,
        commands::{ControlCommand, ControlResponse},
    },
    network::transports::NetworkTransport,
};

/// How long to block on transport receive before running a maintenance cycle.
const POLL_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait between reconnection attempts when the transport is down.
const RECONNECT_DELAY: Duration = Duration::from_secs(15);

pub struct ControlWorker<T: NetworkTransport> {
    dispatchers: HashMap<TetraEntity, CommandDispatcher>,
    transport: T,
    connected: bool,
    last_connect_attempt: Option<Instant>,
}

impl<T: NetworkTransport> ControlWorker<T> {
    pub fn new(dispatchers: HashMap<TetraEntity, CommandDispatcher>, transport: T) -> Self {
        Self {
            dispatchers,
            transport,
            connected: false,
            last_connect_attempt: None,
        }
    }

    pub fn run(&mut self) {
        tracing::debug!("Control worker started");
        self.try_connect();

        loop {
            if self.connected {
                self.poll_commands();
                self.collect_responses();
            } else {
                // Not connected — sleep briefly to avoid busy-spinning
                std::thread::sleep(POLL_TIMEOUT);
            }

            // Detect fresh disconnection
            if !self.transport.is_connected() && self.connected {
                tracing::warn!("Control transport disconnected");
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
    }

    /// Poll the transport for inbound commands, decode them, and dispatch
    /// to the appropriate entity through its [`CommandDispatcher`] link.
    fn poll_commands(&mut self) {
        let msgs = self.transport.receive_reliable();

        for msg in msgs {
            let codec = ControlCodecJson;
            match codec.decode_command(&msg.payload) {
                Ok(command) => {
                    tracing::debug!("command received: {:?}", command);
                    self.dispatch_command(command);
                }
                Err(e) => {
                    tracing::warn!("Command: failed to decode inbound message ({} bytes): {}", msg.payload.len(), e);
                }
            }
        }
    }

    /// Route a command to the correct entity's dispatcher.
    /// Override this mapping as real command variants are added.
    fn dispatch_command(&self, command: ControlCommand) {
        let target = Self::route_control_command(&command);
        match self.dispatchers.get(&target) {
            Some(dispatcher) => {
                tracing::debug!("dispatching command to {:?}", target);
                dispatcher.send(command);
            }
            None => {
                tracing::warn!("no dispatcher registered for {:?}, dropping command", target);
            }
        }
    }

    /// Determine which entity should handle a given command.
    /// Placeholder routing — will be extended as real commands are defined.
    fn route_control_command(command: &ControlCommand) -> TetraEntity {
        match command {
            ControlCommand::SendSds { .. } => TetraEntity::Cmce,
            ControlCommand::CommandA { .. } => TetraEntity::Mm,
            ControlCommand::TestCmdB { .. } => TetraEntity::Cmce,
        }
    }

    /// Drain pending responses from all entity dispatchers and send them
    /// back to the command server.
    fn collect_responses(&mut self) {
        let responses: Vec<ControlResponse> = self.dispatchers.values().flat_map(|d| d.try_recv_responses()).collect();

        for response in &responses {
            tracing::debug!("response collected: {:?}", response);
            self.send_response(response);
        }
    }

    fn send_response(&mut self, response: &ControlResponse) {
        if !self.connected {
            return;
        }

        let codec = ControlCodecJson;
        let payload = codec.encode_response(response);
        if let Err(e) = self.transport.send_reliable(&payload) {
            tracing::warn!("Control transport send failed: {}, will reconnect", e);
            self.connected = false;
            self.try_connect();
        }
    }

    fn try_connect(&mut self) {
        self.last_connect_attempt = Some(Instant::now());
        match self.transport.connect() {
            Ok(()) => {
                tracing::info!("Control transport connected");
                self.connected = true;
            }
            Err(e) => {
                tracing::warn!("Control transport connection failed: {}, will retry in {:?}", e, RECONNECT_DELAY);
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
    use tetra_core::debug::setup_logging_verbose;

    use super::*;
    use crate::net_control::channel::make_control_link;
    use crate::network::transports::mock::MockTransport;

    #[test]
    fn test_route_command_a_to_mm() {
        let target = ControlWorker::<MockTransport>::route_control_command(&ControlCommand::CommandA { handle: 1, parameter: 1 });
        assert_eq!(target, TetraEntity::Mm);
    }

    #[test]
    fn test_route_command_b_to_cmce() {
        let target = ControlWorker::<MockTransport>::route_control_command(&ControlCommand::TestCmdB {
            handle: 2,
            source_ssi: 12345,
            is_group: false,
            payload: vec![],
        });
        assert_eq!(target, TetraEntity::Cmce);
    }

    #[test]
    fn test_worker_dispatches_command_and_collects_response() {
        setup_logging_verbose();

        // Set up per-entity links
        let (mm_dispatcher, mm_endpoint) = make_control_link();
        let mut dispatchers = HashMap::new();
        dispatchers.insert(TetraEntity::Mm, mm_dispatcher);

        // Pre-load a CommandA (routed to Mm) into the mock transport
        let codec = ControlCodecJson;
        let cmd = ControlCommand::CommandA { handle: 1, parameter: 99 };
        let payload = codec.encode_command(&cmd);

        let mut mock = MockTransport::new();
        mock.push_inbound(payload);

        let handle = std::thread::spawn(move || {
            let mut worker = ControlWorker::new(dispatchers, mock);
            worker.try_connect();
            worker.poll_commands();

            // Simulate entity processing: endpoint receives command, sends response
            let received = mm_endpoint.try_recv().expect("Mm should receive CommandA");
            assert!(matches!(received, ControlCommand::CommandA { handle: 1, parameter: 99 }));
            mm_endpoint.respond(ControlResponse::CommandAResponse { handle: 1, result: 99 });

            // Worker collects responses and sends them back over the transport
            worker.collect_responses();

            // Verify the response was sent through the transport
            assert_eq!(worker.transport.sent_payloads().len(), 1);
            let sent = &worker.transport.sent_payloads()[0];
            let decoded = codec.decode_response(sent).unwrap();
            assert!(matches!(decoded, ControlResponse::CommandAResponse { handle: 1, result: 99 }));
        });

        handle.join().expect("command worker panicked");
    }

    #[test]
    fn test_worker_drops_command_without_dispatcher() {
        setup_logging_verbose();

        // No dispatchers registered — command should be dropped with a warning
        let dispatchers = HashMap::new();

        let codec = ControlCodecJson;
        let cmd = ControlCommand::CommandA { handle: 1, parameter: 1 };
        let payload = codec.encode_command(&cmd);

        let mut mock = MockTransport::new();
        mock.push_inbound(payload);

        let handle = std::thread::spawn(move || {
            let mut worker = ControlWorker::new(dispatchers, mock);
            worker.try_connect();
            worker.poll_commands(); // should log warning and not panic
        });

        handle.join().expect("command worker panicked");
    }
}
