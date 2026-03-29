//! Mock transport for testing. Records sent payloads and stubs all receives.

use std::collections::VecDeque;
use std::time::Instant;

use super::{NetworkAddress, NetworkError, NetworkMessage, NetworkTransport};

pub struct MockTransport {
    connected: bool,
    sent: Vec<Vec<u8>>,
    inbound: VecDeque<Vec<u8>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self {
            connected: false,
            sent: Vec::new(),
            inbound: VecDeque::new(),
        }
    }

    pub fn sent_payloads(&self) -> &[Vec<u8>] {
        &self.sent
    }

    /// Queue a raw payload that will be returned by the next `receive_reliable()` call.
    pub fn push_inbound(&mut self, payload: Vec<u8>) {
        self.inbound.push_back(payload);
    }
}

impl NetworkTransport for MockTransport {
    fn connect(&mut self) -> Result<(), NetworkError> {
        self.connected = true;
        Ok(())
    }

    fn send_reliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        if !self.connected {
            return Err(NetworkError::SendFailed("not connected".into()));
        }
        self.sent.push(payload.to_vec());
        Ok(())
    }

    fn send_unreliable(&mut self, payload: &[u8]) -> Result<(), NetworkError> {
        self.send_reliable(payload)
    }

    fn receive_reliable(&mut self) -> Vec<NetworkMessage> {
        self.inbound
            .drain(..)
            .map(|payload| NetworkMessage {
                source: NetworkAddress::Custom {
                    scheme: "mock".into(),
                    address: "test".into(),
                },
                payload,
                timestamp: Instant::now(),
            })
            .collect()
    }

    fn receive_unreliable(&mut self) -> Vec<NetworkMessage> {
        vec![]
    }

    fn wait_for_response_reliable(&mut self) -> Result<NetworkMessage, NetworkError> {
        Err(NetworkError::Timeout)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }
}
