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

    /// Pluto timestamp  configuration
    pub iocfg_pluto: Option<CfgPluto>,
}

impl Default for SoapySdrIoCfg {
    fn default() -> Self {
        Self {
            iocfg_usrpb2xx: None,
            iocfg_limesdr: None,
            iocfg_sxceiver: None,
            iocfg_pluto: None,
        }
    }
}

/// Configuration for Ettus USRP B2xx series
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CfgUsrpB2xx {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
}

/// Configuration for LimeSDR
#[derive(Debug, Clone, Deserialize, Default)]
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
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CfgSxCeiver {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_lna: Option<f64>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_dac: Option<f64>,
    pub tx_gain_mixer: Option<f64>,
}

/// Configuration for Pluto timestamp
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CfgPluto {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
    pub uri: Option<String>,
    pub usb_direct: Option<bool>,
    pub direct: Option<bool>,
    pub timestamp_every: Option<usize>,
    pub loopback: Option<bool>,
}

/// SoapySDR configuration
#[derive(Debug, Clone)]
pub struct CfgSoapySdr {
    /// Uplink frequency in Hz
    pub ul_freq: f64,
    /// Downlink frequency in Hz
    pub dl_freq: f64,
    /// PPM frequency error correction
    pub ppm_err: f64,
    /// Hardware-specific I/O configuration
    pub io_cfg: SoapySdrIoCfg,
}

impl CfgSoapySdr {
    /// Get corrected UL frequency with PPM error applied
    pub fn ul_freq_corrected(&self) -> (f64, f64) {
        let ppm = self.ppm_err;
        let err = (self.ul_freq / 1_000_000.0) * ppm;
        (self.ul_freq + err, err)
    }

    /// Get corrected DL frequency with PPM error applied
    pub fn dl_freq_corrected(&self) -> (f64, f64) {
        let ppm = self.ppm_err;
        let err = (self.dl_freq / 1_000_000.0) * ppm;
        (self.dl_freq + err, err)
    }
}

#[derive(Deserialize)]
pub struct SoapySdrDto {
    pub rx_freq: f64,
    pub tx_freq: f64,
    pub ppm_err: Option<f64>,

    pub iocfg_usrpb2xx: Option<UsrpB2xxDto>,
    pub iocfg_limesdr: Option<LimeSdrDto>,
    pub iocfg_sxceiver: Option<SXceiverDto>,
    pub iocfg_pluto: Option<PlutoDto>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
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

#[derive(Deserialize)]
pub struct PlutoDto {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
    pub uri: Option<String>,
    pub loopback: Option<bool>,
    pub timestamp_every: Option<usize>,
    pub usb_direct: Option<bool>,
    pub direct: Option<bool>,
}
