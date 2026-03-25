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
pub mod sap_fields;
pub mod tdma_time;
pub mod tetra_common;
pub mod tetra_entities;
pub mod timeslot_alloc;
pub mod tx_receipt;
pub mod typed_pdu_fields;

// Re-export commonly used items
pub use address::*;
pub use bitbuffer::BitBuffer;
pub use direction::Direction;
pub use pdu_parse_error::PduParseErr;
pub use phy_types::*;
pub use sap_fields::*;
pub use tdma_time::TdmaTime;
pub use tetra_common::*;
pub use timeslot_alloc::*;
pub use tx_receipt::*;
