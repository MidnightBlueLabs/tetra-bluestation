use serde::Deserialize;

/// Configuration for different SDR hardware devices
#[derive(Debug, Clone, Deserialize)]
pub struct SoapySdrIoCfg {
    /// USRP B2xx series configuration (B200, B210)
    #[serde(default)]
    pub iocfg_usrpb2xx: Option<UsrpB2xxCfg>,

    /// LimeSDR configuration
    #[serde(default)]
    pub iocfg_limesdr: Option<LimeSdrCfg>,

    /// SXceiver configuration
    #[serde(default)]
    pub iocfg_sxceiver: Option<SXceiverCfg>,
}

impl SoapySdrIoCfg {
    pub fn get_soapy_driver_name(&self) -> &'static str {
        if self.iocfg_usrpb2xx.is_some() {
            "uhd"
        } else if self.iocfg_limesdr.is_some() {
            "lime"
        } else if self.iocfg_sxceiver.is_some() {
            "sx"
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
        }
    }
}

/// Configuration for Ettus USRP B2xx series
#[derive(Debug, Clone, Deserialize)]
pub struct UsrpB2xxCfg {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_pga: Option<f64>,
}

/// Configuration for LimeSDR
#[derive(Debug, Clone, Deserialize)]
pub struct LimeSdrCfg {
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
pub struct SXceiverCfg {
    pub rx_ant: Option<String>,
    pub tx_ant: Option<String>,
    pub rx_gain_lna: Option<f64>,
    pub rx_gain_pga: Option<f64>,
    pub tx_gain_dac: Option<f64>,
    pub tx_gain_mixer: Option<f64>,
}

/// SoapySDR configuration
#[derive(Debug, Clone, Deserialize)]
pub struct CfgSoapySdr {
    /// Uplink frequency in Hz
    pub ul_freq: f64,
    /// Downlink frequency in Hz
    pub dl_freq: f64,
    /// PPM frequency error correction
    pub ppm_err: Option<f64>,
    /// Hardware-specific I/O configuration
    pub io_cfg: SoapySdrIoCfg,
}

impl CfgSoapySdr {
    /// Get corrected UL frequency with PPM error applied
    pub fn ul_freq_corrected(&self) -> (f64, f64) {
        let ppm = self.ppm_err.unwrap_or(0.0);
        let err = (self.ul_freq / 1_000_000.0) * ppm;
        (self.ul_freq + err, err)
    }

    /// Get corrected DL frequency with PPM error applied
    pub fn dl_freq_corrected(&self) -> (f64, f64) {
        let ppm = self.ppm_err.unwrap_or(0.0);
        let err = (self.dl_freq / 1_000_000.0) * ppm;
        (self.dl_freq + err, err)
    }
}

impl Default for CfgSoapySdr {
    fn default() -> Self {
        Self {
            ul_freq: 0.0,
            dl_freq: 0.0,
            ppm_err: None,
            io_cfg: SoapySdrIoCfg::default(),
        }
    }
}
