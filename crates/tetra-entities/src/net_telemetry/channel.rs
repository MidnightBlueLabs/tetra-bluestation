use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, unbounded};
use std::time::Duration;

use crate::net_telemetry::events::TelemetryEvent;

// ---------------------------------------------------------------------------
// TelemetrySink  (cloneable, push‑only handle given to entities)
//
// crossbeam Sender is Arc‑backed; cloning is a single atomic increment.
// send() is lock‑free — it claims a slot via atomic FAA and memcpys the
// TelemetryEvent into it.  Small events require zero heap allocation.
// Larger events should use a Box to keep the TelemetryEvent size small
// and avoid heap allocation on send.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct TelemetrySink {
    tx: Sender<TelemetryEvent>,
}

impl TelemetrySink {
    /// Push a telemetry event. Lock‑free. Fire‑and‑forget: silently drops if the receiver is gone.
    #[inline]
    pub fn send(&self, event: TelemetryEvent) {
        let _ = self.tx.send(event);
    }
}

// ---------------------------------------------------------------------------
// TelemetrySource  (receive side, owned by the Telemetry component)
// ---------------------------------------------------------------------------

pub struct TelemetrySource {
    rx: Receiver<TelemetryEvent>,
}

/// Result of a receive-with-timeout operation.
pub enum RecvEvent {
    /// A telemetry event was received.
    Event(TelemetryEvent),
    /// Timed out waiting — channel is still open.
    Timeout,
    /// All sinks were dropped — channel is closed.
    Closed,
}

impl TelemetrySource {
    /// Blocking receive.  Returns `None` when all sinks have been dropped.
    pub fn recv(&self) -> Option<TelemetryEvent> {
        self.rx.recv().ok()
    }

    /// Blocking receive with timeout, distinguishing timeout from channel close.
    pub fn recv_timeout(&self, timeout: Duration) -> RecvEvent {
        match self.rx.recv_timeout(timeout) {
            Ok(event) => RecvEvent::Event(event),
            Err(RecvTimeoutError::Timeout) => RecvEvent::Timeout,
            Err(RecvTimeoutError::Disconnected) => RecvEvent::Closed,
        }
    }

    /// Non-blocking try_recv.
    pub fn try_recv(&self) -> Option<TelemetryEvent> {
        self.rx.try_recv().ok()
    }
}

// ---------------------------------------------------------------------------
// Channel constructor
// ---------------------------------------------------------------------------

/// Create a linked (sink, source) pair.
pub fn telemetry_channel() -> (TelemetrySink, TelemetrySource) {
    let (tx, rx) = unbounded();
    (TelemetrySink { tx }, TelemetrySource { rx })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_two_events() {
        let (sink, source) = telemetry_channel();

        sink.send(TelemetryEvent::MsRegistration { issi: 12345 });

        // Clone the sink (simulating a second entity) and send an Attach event
        let sink2 = sink.clone();
        sink2.send(TelemetryEvent::MsGroupAttach {
            issi: 12345,
            gssis: vec![1, 2, 3],
        });

        // Receive and verify
        let a = source.try_recv().expect("should receive Registration");
        assert!(matches!(a, TelemetryEvent::MsRegistration { issi: 12345 }));

        let b = source.try_recv().expect("should receive Attach");
        if let TelemetryEvent::MsGroupAttach { issi, gssis } = &b {
            assert_eq!(*issi, 12345);
            assert_eq!(*gssis, vec![1, 2, 3]);
        } else {
            panic!("expected Attach variant");
        }

        // No more items
        assert!(source.try_recv().is_none());
    }
}
