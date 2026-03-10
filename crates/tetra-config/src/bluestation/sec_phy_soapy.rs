use serde::Deserialize;
use std::collections::HashMap;
use toml::Value;

/// Configuration for different SDR hardware devices
#[derive(Debug, Clone)]
pub struct SoapySdrIoCfg {
    /// USRP B2xx series configuration (B200, B210)
    pub iocfg_usrpb2xx: Option<CfgUsrpB2xx>,

    /// LimeSDR configuration
    pub iocfg_limesdr: Option<CfgLimeSdr>,

    /// SXceiver configuration
    pub iocfg_sxceiver: Option<CfgSxCeiver>,

    /// ADALM-Pluto (PlutoSDR) configuration
    pub iocfg_plutosdr: Option<CfgPlutoSdr>,
}

impl SoapySdrIoCfg {
    pub fn get_soapy_driver_name(&self) -> &'static str {
        if self.iocfg_usrpb2xx.is_some() {
            "uhd"
        } else if self.iocfg_limesdr.is_some() {
            "lime"
        } else if self.iocfg_sxceiver.is_some() {
            "sx"
        } else if self.iocfg_plutosdr.is_some() {
            "plutosdr"
        } else {
            "unknown"
        }
    }
}

impl Default for SoapySdrIoCfg {
    fn default() -> Self {
        Self {
            iocfg_usrpb2xx: None,
            iocfg_limesdr: None,
            iocfg_sxceiver: None,
            iocfg_plutosdr: None,
        }
    }
}

/// Configuration for Ettus USRP B2xx series
#[derive(Debug, Clone, Deserialize)]
pub struct CfgUsrpB2xx {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
}

/// Configuration for LimeSDR
#[derive(Debug, Clone, Deserialize)]
pub struct CfgLimeSdr {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_lna: Option<f64>,
    pub rx_gain_tia: Option<f64>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pad: Option<f64>,
    pub tx_gain_iamp: Option<f64>,
}

/// Configuration for SXceiver
#[derive(Debug, Clone, Deserialize)]
pub struct CfgSxCeiver {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_lna: Option<f64>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_dac: Option<f64>,
    pub tx_gain_mixer: Option<f64>,
}

/// Configuration for ADALM-Pluto (PlutoSDR)
#[derive(Debug, Clone, Deserialize)]
pub struct CfgPlutoSdr {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
    /// URI for the PlutoSDR (e.g. "usb:" or "ip:192.168.2.1")
    pub uri: Option<String>,
    /// FPGA timestamp insertion interval in samples
    pub timestamp_every: Option<u32>,
    /// Enable USB direct mode (bypasses IIO streaming, uses USB gadget for low-latency timestamped I/O)
    #[serde(default)]
    pub usb_direct: Option<bool>,
    /// Enable FPGA loopback mode (TX→RX) for testing
    #[serde(default)]
    pub loopback: Option<bool>,
}

/// SoapySDR configuration
#[derive(Debug, Clone)]
pub struct CfgSoapySdr {
    /// Uplink frequency in Hz
    pub ul_freq: f64,
    /// Downlink frequency in Hz
    pub dl_freq: f64,
    /// PPM frequency error correction (used for both TX and RX if separate values not set)
    pub ppm_err: f64,
    /// PPM frequency error correction for TX (DL) only. Falls back to ppm_err if None.
    pub ppm_err_tx: Option<f64>,
    /// PPM frequency error correction for RX (UL) only. Falls back to ppm_err if None.
    pub ppm_err_rx: Option<f64>,
    /// Additional TX frequency offset in Hz for fine-tuning TX (applied on top of PPM correction)
    pub tx_freq_offset: f64,
    /// Hardware-specific I/O configuration
    pub io_cfg: SoapySdrIoCfg,
}

impl CfgSoapySdr {
    /// Get corrected UL frequency with PPM error applied
    pub fn ul_freq_corrected(&self) -> (f64, f64) {
        let ppm = self.ppm_err_rx.unwrap_or(self.ppm_err);
        let err = (self.ul_freq / 1_000_000.0) * ppm;
        (self.ul_freq + err, err)
    }

    /// Get corrected DL frequency with PPM error and TX offset applied
    pub fn dl_freq_corrected(&self) -> (f64, f64) {
        let ppm = self.ppm_err_tx.unwrap_or(self.ppm_err);
        let err = (self.dl_freq / 1_000_000.0) * ppm + self.tx_freq_offset;
        (self.dl_freq + err, err)
    }
}

#[derive(Deserialize)]
pub struct SoapySdrDto {
    pub rx_freq: f64,
    pub tx_freq: f64,
    pub ppm_err: Option<f64>,
    pub ppm_err_tx: Option<f64>,
    pub ppm_err_rx: Option<f64>,
    pub tx_freq_offset: Option<f64>,

    pub iocfg_usrpb2xx: Option<UsrpB2xxDto>,
    pub iocfg_limesdr: Option<LimeSdrDto>,
    pub iocfg_sxceiver: Option<SXceiverDto>,
    pub iocfg_plutosdr: Option<PlutoSdrDto>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Deserialize)]
pub struct PlutoSdrDto {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
    pub uri: Option<String>,
    pub timestamp_every: Option<u32>,
    pub usb_direct: Option<bool>,
    pub loopback: Option<bool>,
}

#[derive(Deserialize)]
pub struct UsrpB2xxDto {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
}

#[derive(Deserialize)]
pub struct LimeSdrDto {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_lna: Option<f64>,
    pub rx_gain_tia: Option<f64>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pad: Option<f64>,
    pub tx_gain_iamp: Option<f64>,
}

#[derive(Deserialize)]
pub struct SXceiverDto {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_lna: Option<f64>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_dac: Option<f64>,
    pub tx_gain_mixer: Option<f64>,
}
