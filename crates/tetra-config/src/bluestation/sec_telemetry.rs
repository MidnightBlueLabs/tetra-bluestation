use std::collections::HashMap;

use serde::Deserialize;
use toml::Value;

/// Telemetry endpoint configuration
#[derive(Debug, Clone)]
pub struct CfgTelemetry {
    /// Telemetry server hostname or IP
    pub host: String,
    /// Telemetry server port
    pub port: u16,
    /// Use TLS (wss://)
    pub use_tls: bool,
    /// Optional path to a DER-encoded CA certificate for self-signed TLS
    pub ca_cert: Option<String>,
    /// Optional (username, password) for HTTP Basic authentication
    pub credentials: Option<(String, String)>,
}

#[derive(Deserialize)]
pub struct CfgTelemetryDto {
    /// Telemetry server hostname or IP
    pub host: String,
    /// Telemetry server port
    pub port: u16,
    /// Use TLS (wss://)
    #[serde(default)]
    pub use_tls: bool,
    /// Optional path to a DER-encoded CA certificate for self-signed TLS
    pub ca_cert: Option<String>,
    /// Optional username for HTTP Basic auth
    pub username: Option<String>,
    /// Optional password for HTTP Basic auth
    pub password: Option<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Convert a [`CfgTelemetryDto`] (from TOML) into a [`CfgTelemetry`].
///
/// Returns an error string if `ca_cert` is set but `use_tls` is `false`.
pub fn apply_telemetry_patch(src: CfgTelemetryDto) -> Result<CfgTelemetry, String> {
    if src.ca_cert.is_some() && !src.use_tls {
        return Err("telemetry: ca_cert requires use_tls = true".to_string());
    }

    Ok(CfgTelemetry {
        host: src.host,
        port: src.port,
        use_tls: src.use_tls,
        credentials: match (src.username, src.password) {
            (Some(u), Some(p)) => Some((u, p)),
            (None, None) => None,
            _ => return Err("telemetry: both username and password must be set for credentials".to_string()),
        },
        ca_cert: src.ca_cert,
    })
}
