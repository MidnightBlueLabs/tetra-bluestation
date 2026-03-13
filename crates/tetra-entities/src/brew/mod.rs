//! Brew protocol integration for TETRA group call bridging via TetraPack/BrandMeister WebSocket API

pub mod components;
pub mod entity;
pub mod protocol;
pub mod worker;

pub use components::brew_routable::feature_sds_enabled;
/// Convenience re-export of commonly externally used functions
pub use components::brew_routable::is_active;
pub use components::brew_routable::is_brew_gssi_routable;
pub use components::brew_routable::is_brew_issi_routable;
pub use components::brew_routable::is_tetrapack_sds_service_issi;
