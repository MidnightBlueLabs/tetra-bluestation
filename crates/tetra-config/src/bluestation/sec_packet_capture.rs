use serde::Deserialize;

/// Packet capture configuration
#[derive(Debug, Clone)]
pub struct CfgPacketCapture {
    /// Packet capture server hostname or IP
    pub host: String,
    /// Packet capture UDP server port
    pub port: u16,
}

#[derive(Deserialize)]
pub struct CfgPacketCaptureDto {
    /// Packet capture server hostname or IP
    pub host: String,
    /// Packet capture UDP server port
    pub port: u16,
}

/// Convert a [`CfgPacketCaptureDto`] (from TOML) into a [`CfgPacketCapture`].
pub fn apply_packet_capture_patch(src: CfgPacketCaptureDto) -> Result<CfgPacketCapture, String> {
    Ok(CfgPacketCapture {
        host: src.host,
        port: src.port,
    })
}
