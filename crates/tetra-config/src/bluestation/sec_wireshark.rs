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
    /// Optional PCAP file path for saving synthetic UDP capture packets
    pub pcap_file: Option<String>,
    /// Suppress AACH/ACCESS-ASSIGN packets from Wireshark export
    pub suppress_access_assign: bool,
    /// Suppress BSCH/D-MLE-SYNC packets from Wireshark export
    pub suppress_d_mle_sync: bool,
    /// Suppress BNCH/D-MLE-SYSINFO packets from Wireshark export
    pub suppress_d_mle_sysinfo: bool,
}

#[derive(Deserialize)]
pub struct CfgWiresharkDto {
    /// Wireshark host or IP address
    pub host: String,
    /// UDP destination port
    pub port: u16,
    /// Optional PCAP file path for saving synthetic UDP capture packets
    pub pcap_file: Option<String>,
    /// Suppress AACH/ACCESS-ASSIGN packets from Wireshark export
    pub suppress_access_assign: Option<bool>,
    /// Suppress BSCH/D-MLE-SYNC packets from Wireshark export
    pub suppress_d_mle_sync: Option<bool>,
    /// Suppress BNCH/D-MLE-SYSINFO packets from Wireshark export
    pub suppress_d_mle_sysinfo: Option<bool>,

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
    if let Some(ref pcap_file) = src.pcap_file
        && pcap_file.trim().is_empty()
    {
        return Err("wireshark: pcap_file must not be empty".to_string());
    }

    Ok(CfgWireshark {
        host: src.host,
        port: src.port,
        pcap_file: src.pcap_file,
        suppress_access_assign: src.suppress_access_assign.unwrap_or(false),
        suppress_d_mle_sync: src.suppress_d_mle_sync.unwrap_or(false),
        suppress_d_mle_sysinfo: src.suppress_d_mle_sysinfo.unwrap_or(false),
    })
}
