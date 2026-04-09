use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use std::time::Duration;

use crate::net_wireshark::events::Type1Capture;

#[derive(Clone)]
pub struct WiresharkSink {
    tx: Sender<Type1Capture>,
}

impl WiresharkSink {
    #[inline]
    pub fn send(&self, frame: Type1Capture) {
        let _ = self.tx.send(frame);
    }
}

pub struct WiresharkSource {
    rx: Receiver<Type1Capture>,
}

pub enum RecvFrame {
    Frame(Type1Capture),
    Timeout,
    Closed,
}

impl WiresharkSource {
    pub fn recv_timeout(&self, timeout: Duration) -> RecvFrame {
        match self.rx.recv_timeout(timeout) {
            Ok(frame) => RecvFrame::Frame(frame),
            Err(RecvTimeoutError::Timeout) => RecvFrame::Timeout,
            Err(RecvTimeoutError::Disconnected) => RecvFrame::Closed,
        }
    }
}

pub fn wireshark_channel() -> (WiresharkSink, WiresharkSource) {
    let (tx, rx) = unbounded();
    (WiresharkSink { tx }, WiresharkSource { rx })
}
