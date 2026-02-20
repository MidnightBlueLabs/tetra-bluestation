//! TETRA configuration management
//!
//! This crate provides configuration loading and parsing for TETRA BlueStation:
//! - TOML configuration file parsing
//! - Stack configuration structures
//! - SoapySDR-specific configuration

pub mod stack_config;
pub mod stack_config_brew;
pub mod stack_config_soapy;
//pub mod timeslot_alloc;
pub mod toml_config;

pub use stack_config::*;
//pub use timeslot_alloc::*;
pub use toml_config::*;
