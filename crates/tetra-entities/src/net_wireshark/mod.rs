//! External Wireshark capture emitter.
//!
//! Runs outside the real-time core in its own thread. Receives decoded Type-1
//! bitstreams from the core and forwards them over a pluggable transport.

pub mod channel;
pub mod events;
pub mod pcap;
pub mod protocol;
pub mod worker;

pub use self::channel::{WiresharkSink, WiresharkSource, wireshark_channel};
pub use self::events::{Type1Capture, Type1Direction};
pub use self::worker::WiresharkWorker;
