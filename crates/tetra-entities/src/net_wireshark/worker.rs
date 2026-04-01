use std::time::{Duration, Instant};

use crate::{
    net_wireshark::{
        channel::{RecvFrame, WiresharkSource},
        events::Type1Capture,
        pcap::PcapWriter,
        protocol::encode_datagram,
    },
    network::transports::NetworkTransport,
};

const POLL_TIMEOUT: Duration = Duration::from_millis(500);
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

pub struct WiresharkWorker<T: NetworkTransport> {
    source: WiresharkSource,
    transport: T,
    pcap: Option<PcapWriter<std::io::BufWriter<std::fs::File>>>,
    connected: bool,
    last_connect_attempt: Option<Instant>,
}

impl<T: NetworkTransport> WiresharkWorker<T> {
    pub fn new(source: WiresharkSource, transport: T) -> Self {
        Self {
            source,
            transport,
            pcap: None,
            connected: false,
            last_connect_attempt: None,
        }
    }

    pub fn with_pcap(mut self, pcap: PcapWriter<std::io::BufWriter<std::fs::File>>) -> Self {
        self.pcap = Some(pcap);
        self
    }

    pub fn run(&mut self) {
        tracing::debug!("Wireshark worker started");
        self.try_connect();

        loop {
            match self.source.recv_timeout(POLL_TIMEOUT) {
                RecvFrame::Frame(frame) => {
                    self.forward_frame(&frame);
                }
                RecvFrame::Timeout => {}
                RecvFrame::Closed => {
                    tracing::debug!("Wireshark worker: all sinks dropped, shutting down");
                    break;
                }
            }

            if self.connected {
                for msg in self.transport.receive_unreliable() {
                    tracing::trace!(
                        "Wireshark transport received unexpected datagram ({} bytes) from {:?}",
                        msg.payload.len(),
                        msg.source
                    );
                }
            }

            if !self.transport.is_connected() && self.connected {
                tracing::warn!("Wireshark transport disconnected");
                self.connected = false;
            }

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
        tracing::info!("Wireshark worker exiting");
    }

    fn forward_frame(&mut self, frame: &Type1Capture) {
        let payload = match encode_datagram(frame) {
            Ok(payload) => payload,
            Err(e) => {
                tracing::warn!("Wireshark frame encoding failed: {}", e);
                return;
            }
        };

        if let Some(pcap) = &mut self.pcap
            && let Err(e) = pcap.write_datagram(&payload)
        {
            tracing::warn!("Wireshark PCAP write failed: {}, disabling PCAP output", e);
            self.pcap = None;
        }

        if !self.connected {
            self.try_connect();
            if !self.connected {
                return;
            }
        }

        if let Err(e) = self.transport.send_unreliable(&payload) {
            tracing::warn!("Wireshark transport send failed: {}, will reconnect", e);
            self.connected = false;
            self.try_connect();
        }
    }

    fn try_connect(&mut self) {
        self.last_connect_attempt = Some(Instant::now());
        match self.transport.connect() {
            Ok(()) => {
                tracing::info!("Wireshark transport connected");
                self.connected = true;
            }
            Err(e) => {
                tracing::warn!("Wireshark transport connection failed: {}, will retry in {:?}", e, RECONNECT_DELAY);
                self.connected = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::{BitBuffer, PhyBlockNum, TdmaTime};
    use tetra_saps::tmv::enums::logical_chans::LogicalChannel;

    use super::*;
    use crate::net_wireshark::{Type1Direction, wireshark_channel};
    use crate::network::transports::mock::MockTransport;

    #[test]
    fn test_worker_forwards_frames_and_exits() {
        let (sink, source) = wireshark_channel();

        let handle = std::thread::spawn(move || {
            let mut worker = WiresharkWorker::new(source, MockTransport::new());
            worker.run();
        });

        sink.send(Type1Capture {
            direction: Type1Direction::Downlink,
            logical_channel: LogicalChannel::SchF,
            block_num: PhyBlockNum::Both,
            crc_pass: true,
            time: TdmaTime::default(),
            scrambling_code: 0x12345678,
            bits: BitBuffer::from_bitstr("1010"),
        });

        drop(sink);
        handle.join().expect("wireshark worker panicked");
    }
}
