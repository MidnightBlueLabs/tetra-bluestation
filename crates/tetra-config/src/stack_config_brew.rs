use std::collections::HashMap;

use serde::Deserialize;
use tetra_core::ranges::SortedDisjointSsiRanges;
use toml::Value;

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
    /// ISSI to register with the TetraPack server
    pub issi: u32,
    /// Reconnection delay in seconds
    pub reconnect_delay_secs: u64,
    /// Extra initial jitter playout delay in frames (added on top of adaptive baseline)
    pub jitter_initial_latency_frames: u8,

    pub whitelisted_ssi_ranges: Option<SortedDisjointSsiRanges>,
    pub blacklisted_ssi_ranges: Option<SortedDisjointSsiRanges>,
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
    /// Reconnection delay in seconds
    #[serde(default = "default_brew_reconnect_delay")]
    pub reconnect_delay_secs: u64,
    /// Extra initial jitter playout delay in frames (added on top of adaptive baseline)
    #[serde(default)]
    pub jitter_initial_latency_frames: u8,

    pub whitelisted_ssi_ranges: Option<Vec<(u32, u32)>>,
    pub blacklisted_ssi_ranges: Option<Vec<(u32, u32)>>,

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
    let whitelist = src
        .whitelisted_ssi_ranges
        .map(|ranges| SortedDisjointSsiRanges::from_vec_tuple(ranges));
    let blacklist = src
        .blacklisted_ssi_ranges
        .map(|ranges| SortedDisjointSsiRanges::from_vec_tuple(ranges));
    assert!(
        !(whitelist.is_some() && blacklist.is_some()),
        "Cannot specify both whitelisted_ssi_ranges and blacklisted_ssi_ranges in brew config"
    );
    CfgBrew {
        host: src.host,
        port: src.port,
        tls: src.tls,
        username: Some(src.username.to_string()),
        password: Some(src.password),
        issi: src.issi,
        reconnect_delay_secs: src.reconnect_delay_secs,
        jitter_initial_latency_frames: src.jitter_initial_latency_frames,
        whitelisted_ssi_ranges: whitelist,
        blacklisted_ssi_ranges: blacklist,
    }
}
