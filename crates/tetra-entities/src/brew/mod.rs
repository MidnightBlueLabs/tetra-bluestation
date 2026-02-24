//! Brew protocol integration for TETRA group call bridging via TetraPack/BrandMeister WebSocket API

pub mod components;
pub mod entity;
pub mod protocol;
pub mod worker;

/// Convenience re-export of commonly externally used functions
pub use components::brew_routable::is_active;
pub use components::brew_routable::is_brew_routable;
