use std::collections::HashMap;

use serde::Deserialize;
use toml::Value;

/// Wireshark UDP capture endpoint configuration
#[derive(Debug, Clone)]
pub struct CfgWireshark {
    /// Wireshark host or IP address
    pub host: String,
    /// UDP destination port
    pub port: u16,
}

#[derive(Deserialize)]
pub struct CfgWiresharkDto {
    /// Wireshark host or IP address
    pub host: String,
    /// UDP destination port
    pub port: u16,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Convert a [`CfgWiresharkDto`] (from TOML) into a [`CfgWireshark`].
pub fn apply_wireshark_patch(src: CfgWiresharkDto) -> Result<CfgWireshark, String> {
    if src.host.trim().is_empty() {
        return Err("wireshark: host must not be empty".to_string());
    }
    if src.port == 0 {
        return Err("wireshark: port must be non-zero".to_string());
    }

    Ok(CfgWireshark {
        host: src.host,
        port: src.port,
    })
}
