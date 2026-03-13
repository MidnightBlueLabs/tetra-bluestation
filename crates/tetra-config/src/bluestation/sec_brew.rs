use std::{collections::HashMap, time::Duration};

use serde::Deserialize;
use toml::Value;

fn default_brew_feature_sds_enabled() -> bool {
    true
}

/// Brew protocol (TetraPack/BrandMeister) configuration
#[derive(Debug, Clone)]
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
    /// Reconnection delay
    pub reconnect_delay: Duration,
    /// Extra initial jitter playout delay in frames (added on top of adaptive baseline)
    pub jitter_initial_latency_frames: u8,

    /// Set to true when SDS between local and Brew clients is enabled
    pub feature_sds_enabled: bool,
    /// If present, restrict Brew call to these remote SSIs
    pub whitelisted_ssis: Option<Vec<u32>>,
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
    /// Reconnection delay in seconds
    #[serde(default = "default_brew_reconnect_delay")]
    pub reconnect_delay_secs: u64,
    /// Extra initial jitter playout delay in frames (added on top of adaptive baseline)
    #[serde(default)]
    pub jitter_initial_latency_frames: u8,

    /// If present, restrict Brew call to these remote SSIs
    pub whitelisted_ssis: Option<Vec<u32>>,

    /// Set to true when SDS between local and Brew clients is enabled
    #[serde(default = "default_brew_feature_sds_enabled")]
    pub feature_sds_enabled: bool,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn default_brew_port() -> u16 {
    443
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
        reconnect_delay: Duration::from_secs(src.reconnect_delay_secs),
        jitter_initial_latency_frames: src.jitter_initial_latency_frames,
        feature_sds_enabled: src.feature_sds_enabled,
        whitelisted_ssis: src.whitelisted_ssis,
    }
}
