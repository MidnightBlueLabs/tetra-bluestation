use std::collections::HashMap;

use serde::Deserialize;
use toml::Value;

/// Brew protocol (TetraPack/BrandMeister) configuration
#[derive(Debug, Clone, Deserialize)]
pub struct CfgBrew {
    /// TetraPack server hostname or IP
    pub host: String,
    /// TetraPack server port
    pub port: u16,
    /// Use TLS (wss:// / https://)
    pub tls: bool,
    /// Optional username for HTTP Digest auth
    pub username: Option<String>,
    /// Optional password for HTTP Digest auth
    pub password: Option<String>,
    /// ISSI to register with the TetraPack server
    pub issi: u32,
    /// GSSIs (group IDs) to affiliate to
    pub groups: Vec<u32>,
    /// Reconnection delay in seconds
    pub reconnect_delay_secs: u64,
}

#[derive(Default, Deserialize)]
pub struct CfgBrewDto {
    /// TetraPack server hostname or IP
    pub host: String,
    /// TetraPack server port
    #[serde(default = "default_brew_port")]
    pub port: u16,
    /// Use TLS (wss:// / https://)
    pub tls: bool,
    /// Optional username for HTTP Digest auth
    pub username: u32,
    /// Optional password for HTTP Digest auth
    pub password: String,
    /// ISSI to register with the TetraPack server
    pub issi: u32,
    /// GSSIs (group IDs) to affiliate to
    pub groups: Vec<u32>,
    /// Reconnection delay in seconds
    #[serde(default = "default_brew_reconnect_delay")]
    pub reconnect_delay_secs: u64,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn default_brew_port() -> u16 {
    3000
}

fn default_brew_reconnect_delay() -> u64 {
    15
}

/// Convert a CfgBrewDto (from TOML) into a CfgBrew (used in the stack config)
pub fn apply_brew_patch(src: CfgBrewDto) -> CfgBrew {
    CfgBrew {
        host: src.host,
        port: src.port,
        tls: src.tls,
        username: Some(src.username.to_string()),
        password: Some(src.password),
        issi: src.issi,
        groups: src.groups,
        reconnect_delay_secs: src.reconnect_delay_secs,
    }
}
