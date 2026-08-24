//! Subscriber signalling from MM to CMCE and the network bridge (Brew).
//!
//! MM owns the subscriber store; an event names a subscriber and what happened to it, and the
//! receivers only react. Anything else about the subscriber is read back from the global state.
//!
//! A subscriber that leaves the cell is announced as [`MmSubscriberEvent::Deaffiliate`] for its
//! groups followed by [`MmSubscriberEvent::Deregister`]: once deregistered the store no longer
//! knows which groups it held, so receivers that care about the groups act on the first event.

/// MM to CMCE and Brew: what happened to a subscriber.
#[derive(Debug, Clone)]
pub enum MmSubscriberEvent {
    /// The subscriber attached to this cell. It holds no groups yet.
    Register { issi: u32 },
    /// The subscriber left the cell. Its groups were announced by a preceding `Deaffiliate`.
    Deregister { issi: u32 },
    /// The subscriber joined these groups.
    Affiliate { issi: u32, groups: Vec<u32> },
    /// The subscriber left these groups.
    Deaffiliate { issi: u32, groups: Vec<u32> },
}
