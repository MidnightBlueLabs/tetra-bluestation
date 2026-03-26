//! Device-specific SoapySDR settings

use tetra_config::bluestation::{StackMode, sec_phy_soapy::*};

/// Enum of all supported devices
pub enum SupportedDevice {
    LimeSdr(LimeSdrModel),
    SXceiver,
    PlutoSdr,
    Usrp(UsrpModel),
}

#[derive(Debug, PartialEq)]
pub enum LimeSdrModel {
    LimeSdrUsb,
    LimeSdrMiniV2,
    LimeNetMicro,
    /// Other LimeSDR models with FX3 driver
    OtherFx3,
    /// Other LimeSDR models with FT601 driver
    OtherFt601,
}

#[derive(Debug, PartialEq)]
pub enum UsrpModel {
    B200,
    B210,
    Other,
}

impl SupportedDevice {
    /// Detect an SDR device based on driver key and hardware key.
    /// Return None if the device is not supported.
    pub fn detect(driver_key: &str, hardware_key: &str) -> Option<Self> {
        match (driver_key, hardware_key) {
            ("FX3", "LimeSDR-USB") => Some(Self::LimeSdr(LimeSdrModel::LimeSdrUsb)),
            ("FX3", _) => Some(Self::LimeSdr(LimeSdrModel::OtherFx3)),

            ("FT601", "LimeSDR-Mini_v2") => Some(Self::LimeSdr(LimeSdrModel::LimeSdrMiniV2)),
            ("FT601", "LimeNET-Micro") => Some(Self::LimeSdr(LimeSdrModel::LimeNetMicro)),
            ("FT601", _) => Some(Self::LimeSdr(LimeSdrModel::OtherFt601)),

            ("sx", _) => Some(Self::SXceiver),

            ("PlutoSDR", _) => Some(Self::PlutoSdr),

            // USRP B210 seems to report as ("b200", "B210"),
            // but the driver key is also known to be "uhd" in some cases.
            // The reason is unknown but might be due to
            // gateware, firmware or driver version differences.
            // Try to detect USRP correctly in all cases.
            ("b200", "B200") | ("uhd", "B200") => Some(Self::Usrp(UsrpModel::B200)),
            ("b200", "B210") | ("uhd", "B210") => Some(Self::Usrp(UsrpModel::B210)),
            ("b200", _) | ("uhd", _) => Some(Self::Usrp(UsrpModel::Other)),
            // TODO: add other USRP models if needed
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SdrSettings {
    /// Settings template, holding which SDR is used
    pub name: String,
    /// If false, timestamp of latest RX read is used to estimate
    /// current hardware time. This is used in case get_hardware_time
    /// is unacceptably slow or not supported.
    pub use_get_hardware_time: bool,
    /// Receive and transmit sample rate.
    pub fs: f64,
    /// Receive channel number
    pub rx_ch: usize,
    /// Transmit channel number
    pub tx_ch: usize,
    /// Receive antenna
    pub rx_ant: Option<String>,
    /// Transmit antenna
    pub tx_ant: Option<String>,
    /// Receive gains
    pub rx_gain: Vec<(String, f64)>,
    /// Transmit gains
    pub tx_gain: Vec<(String, f64)>,

    /// Receive stream arguments
    pub rx_args: Vec<(String, String)>,
    /// Transmit stream arguments
    pub tx_args: Vec<(String, String)>,

    /// Additional device arguments
    pub dev_args: Vec<(String, String)>,
}

pub enum Error {
    InvalidConfiguration,
}

impl SdrSettings {
    /// Get settings based on SDR type and SoapySDR configuration
    pub fn get_settings(cfg: &CfgSoapySdr, device: SupportedDevice, mode: StackMode) -> Result<Self, Error> {
        let mut settings = Self::get_defaults(cfg, device, mode);

        // Override settings if specified in configuration
        if let Some(fs) = cfg.fs {
            settings.fs = fs;
        }
        if let Some(ch) = cfg.rx_ch {
            settings.rx_ch = ch;
        }
        if let Some(ch) = cfg.tx_ch {
            settings.tx_ch = ch;
        }
        if let Some(ant) = &cfg.rx_ant {
            settings.rx_ant = Some(ant.clone());
        }
        if let Some(ant) = &cfg.tx_ant {
            settings.tx_ant = Some(ant.clone());
        }

        let mut cfg_gains = cfg.rx_gains.clone();
        for (name, value) in settings.rx_gain.iter_mut() {
            if let Some(gain) = cfg_gains.remove(&(*name.to_lowercase())) {
                *value = gain;
            }
        }
        if !cfg_gains.is_empty() {
            tracing::error!("Unsupported RX gains for {}: {:?}", settings.name, cfg_gains);
            return Err(Error::InvalidConfiguration);
        }

        let mut cfg_gains = cfg.tx_gains.clone();
        for (name, value) in settings.tx_gain.iter_mut() {
            if let Some(gain) = cfg_gains.remove(&(*name.to_lowercase())) {
                *value = gain;
            }
        }
        if !cfg_gains.is_empty() {
            tracing::error!("Unsupported TX gains for {}: {:?}", settings.name, cfg_gains);
            return Err(Error::InvalidConfiguration);
        }

        // TODO: check for extra gain fields in cfg

        Ok(settings)
    }

    /// Get default settings based on SDR type
    fn get_defaults(cfg: &CfgSoapySdr, device: SupportedDevice, mode: StackMode) -> Self {
        match device {
            SupportedDevice::LimeSdr(model) => Self::settings_limesdr(mode, model),

            SupportedDevice::SXceiver => Self::settings_sxceiver(mode, cfg.fs),

            SupportedDevice::PlutoSdr => Self::settings_pluto(mode),

            SupportedDevice::Usrp(model) => Self::settings_usrp(mode, model),
        }
    }

    /// Reasonable defaults for many SDR devices.
    /// These should not be directly used for any device
    /// but are useful as a template for the most common settings.
    /// This reduces changed needed in code in case
    /// more fields are added to SdrSettings to handle some special cases.
    fn default(mode: StackMode) -> Self {
        Self {
            name: String::new(), // should be always overridden

            // With FCFB bin spacing of 500 Hz and overlap factor or 1/4,
            // FFT size becomes fs/500 and must be a multiple of 4.
            // If possible, use a power-of-two value in kHz
            // because power-of-two FFT sizes are most computationally efficient.
            fs: match mode {
                // 512 kHz is enough for BS use,
                // and some devices struggle with very low sample rates
                // lower than that, making it a good default choice.
                StackMode::Bs | StackMode::Ms => 512e3,
                // Simultaneous UL/DL monitoring at 10 MHz duplex spacing
                // needs something well above 10 MHz.
                StackMode::Mon => 16384e3,
            },

            use_get_hardware_time: true,
            rx_ant: None,
            tx_ant: None,
            rx_gain: vec![],
            tx_gain: vec![],
            rx_ch: 0,
            tx_ch: 0,
            rx_args: vec![],
            tx_args: vec![],
            dev_args: vec![],
        }
    }

    fn settings_limesdr(mode: StackMode, model: LimeSdrModel) -> Self {
        Self {
            name: match model {
                LimeSdrModel::LimeSdrUsb => "LimeSDR USB",
                LimeSdrModel::LimeSdrMiniV2 => "LimeSDR Mini 2.0",
                LimeSdrModel::LimeNetMicro => "LimeNET Micro",
                LimeSdrModel::OtherFx3 => "Unknown LimeSDR model with FX3",
                LimeSdrModel::OtherFt601 => "Unknown LimeSDR model with FT601",
            }
            .to_string(),

            rx_ant: Some(
                match model {
                    LimeSdrModel::LimeSdrUsb => "LNAL",
                    _ => "LNAW",
                }
                .to_string(),
            ),

            tx_ant: Some(
                match model {
                    LimeSdrModel::LimeSdrUsb => "BAND1",
                    _ => "BAND2",
                }
                .to_string(),
            ),

            rx_gain: vec![("LNA".to_string(), 18.0), ("TIA".to_string(), 6.0), ("PGA".to_string(), 10.0)],
            tx_gain: vec![("PAD".to_string(), 22.0), ("IAMP".to_string(), 6.0)],

            // Minimum latency for BS/MS, maximum throughput for monitor
            rx_args: vec![("latency".to_string(), if mode == StackMode::Mon { "1" } else { "0" }.to_string())],
            tx_args: vec![("latency".to_string(), if mode == StackMode::Mon { "1" } else { "0" }.to_string())],

            ..Self::default(mode)
        }
    }

    fn settings_sxceiver(mode: StackMode, fs_override: Option<f64>) -> Self {
        // TODO: pass detected clock rate or list of supported sample rates
        // to get_settings and choose sample rate accordingly.
        // Ok, it is not strictly needed now that sample rate can be overridden.
        // That added another minor issue, though:
        // sample rate affects the optimal period size
        // and override is applied after it is computed.
        // OK, duplicate handle sample rate override here
        // as an ugly little extra special case...
        let fs = fs_override.unwrap_or(600e3);
        Self {
            name: "SXceiver".to_string(),
            fs,

            rx_ant: Some("RX".to_string()),
            tx_ant: Some("TX".to_string()),

            rx_gain: vec![("LNA".to_string(), 42.0), ("PGA".to_string(), 16.0)],
            tx_gain: vec![("DAC".to_string(), 9.0), ("MIXER".to_string(), 30.0)],

            rx_args: vec![("period".to_string(), block_size(fs).to_string())],
            tx_args: vec![("period".to_string(), block_size(fs).to_string())],

            ..Self::default(mode)
        }
    }

    fn settings_usrp(mode: StackMode, model: UsrpModel) -> Self {
        Self {
            name: match model {
                UsrpModel::B200 => "USRP B200",
                UsrpModel::B210 => "USRP B210",
                UsrpModel::Other => "Unknown USRP model",
            }
            .to_string(),

            rx_ant: Some("TX/RX".to_string()),
            tx_ant: Some("TX/RX".to_string()),

            rx_gain: vec![("PGA".to_string(), 50.0)],
            tx_gain: vec![("PGA".to_string(), 35.0)],

            ..Self::default(mode)
        }
    }

    fn settings_pluto(mode: StackMode) -> Self {
        Self {
            name: "Pluto".to_string(),
            // get_hardware_time is apparently not implemented for pluto.
            use_get_hardware_time: false,

            // TODO: check if sample rate could be increased to 1024e3.
            // That would allow a power-of-two FFT size for lower CPU use.
            fs: 1e6,

            rx_ant: Some("A_BALANCED".to_string()),
            tx_ant: Some("A".to_string()),

            rx_gain: vec![("PGA".to_string(), 20.0)],
            tx_gain: vec![("PGA".to_string(), 89.0)],

            dev_args: vec![
                ("direct".to_string(), "1".to_string()),
                ("timestamp_every".to_string(), "1500".to_string()),
                ("loopback".to_string(), "0".to_string()),
            ],

            ..Self::default(mode)
        }
    }
}

/// Get processing block size in samples for a given sample rate.
/// This can be used to optimize performance for some SDRs.
pub fn block_size(fs: f64) -> usize {
    // With current FCFB parameters processing blocks are 1.5 ms long.
    // It is a bit bug prone to have it here in case
    // FCFB parameters are changed, but it makes things simpler for now.
    (fs * 1.5e-3).round() as usize
}
