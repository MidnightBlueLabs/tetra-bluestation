//! Core utilities for TETRA BlueStation
//!
//! This crate provides fundamental types and utilities used across the TETRA stack

/// Git version string, set at compile time
pub const GIT_VERSION: &str = git_version::git_version!(fallback = "unknown");
/// Stack version followed by git version string, e.g., "0.1.0-aabbccdd"
pub const STACK_VERSION: &str = const_format::formatcp!("{}-{}", env!("CARGO_PKG_VERSION"), GIT_VERSION);

pub mod address;
pub mod bitbuffer;
pub mod debug;
pub mod direction;
pub mod freqs;
pub mod pdu_parse_error;
pub mod phy_types;
pub mod ranges;
pub mod tdma_time;
pub mod tetra_common;
pub mod tetra_entities;
pub mod timeslot_alloc;
pub mod typed_pdu_fields;

// Re-export commonly used items
pub use address::*;
pub use bitbuffer::BitBuffer;
pub use direction::Direction;
pub use pdu_parse_error::PduParseErr;
pub use phy_types::*;
pub use tdma_time::TdmaTime;
pub use tetra_common::*;
pub use timeslot_alloc::*;

/// Handle assigned by MLE to primitives for MM/CMCE/SNDCP
pub type MleHandle = u32;

pub type LinkId = u32;

/// The endpoint identifiers between the MLE and LLC, and between the LLC and MAC, refer to the MAC resource that is
/// currently used for that service. These identifiers may be local. There shall be a unique correspondence between the
/// endpoint identifier and the physical allocation (timeslot or timeslots) used in the MAC. (This correspondence is known
/// only within the MAC.) More than one advanced link may use one MAC resource.
/// In the current implementation, the endpoint_id is just the timeslot number used by the MAC.
pub type EndpointId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalChannel {
    Tp,
    Cp,
    Unallocated,
}
