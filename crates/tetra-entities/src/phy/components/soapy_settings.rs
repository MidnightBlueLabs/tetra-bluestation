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
            ("FX3", "LimeSDR-USB") =>
                Some(Self::LimeSdr(LimeSdrModel::LimeSdrUsb)),
            ("FX3", _) =>
                Some(Self::LimeSdr(LimeSdrModel::OtherFx3)),

            ("FT601", "LimeSDR-Mini_v2") =>
                Some(Self::LimeSdr(LimeSdrModel::LimeSdrMiniV2)),
            ("FT601", "LimeNET-Micro") =>
                Some(Self::LimeSdr(LimeSdrModel::LimeNetMicro)),
            ("FT601", _) =>
                Some(Self::LimeSdr(LimeSdrModel::OtherFt601)),

            ("sx", _) =>
                Some(Self::SXceiver),

            ("PlutoSDR", _) =>
                Some(Self::PlutoSdr),

            // USRP B210 seems to report as ("b200", "B210"),
            // but the driver key is also known to be "uhd" in some cases.
            // The reason is unknown but might be due to
            // gateware, firmware or driver version differences.
            // Try to detect USRP correctly in all cases.
            ("b200", "B200") | ("uhd", "B200") =>
                Some(Self::Usrp(UsrpModel::B200)),
            ("b200", "B210") | ("uhd", "B210") =>
                Some(Self::Usrp(UsrpModel::B210)),
            ("b200", _) | ("uhd", _) =>
                Some(Self::Usrp(UsrpModel::Other)),
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

impl SdrSettings {
    /// Get settings based on SDR type
    pub fn get_settings(io_cfg: &SoapySdrIoCfg, device: SupportedDevice, mode: StackMode) -> Self {
        match device {
            SupportedDevice::LimeSdr(model) =>
                Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, model),

            SupportedDevice::SXceiver =>
                Self::settings_sxceiver(&io_cfg.iocfg_sxceiver, mode),

            SupportedDevice::PlutoSdr =>
                Self::settings_pluto(&io_cfg.iocfg_pluto, mode),

            SupportedDevice::Usrp(model) =>
                Self::settings_usrp(&io_cfg.iocfg_usrpb2xx, mode, model),
        }
    }

    /// Reasonable defaults for many SDR devices.
    /// These should not be directly used for any device
    /// but are useful as a template for the most common settings.
    /// This reduces changed needed in code in case
    /// more fields are added to SdrSettings to handle some special cases.
    fn default(mode: StackMode) -> Self {
        Self {
            name: "".to_string(), // should be always overridden

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
            rx_args: vec![],
            tx_args: vec![],
            dev_args: vec![],
        }
    }

    fn settings_limesdr(cfg: &Option<CfgLimeSdr>, mode: StackMode, model: LimeSdrModel) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgLimeSdr::default() };

        SdrSettings {
            name: match model {
                LimeSdrModel::LimeSdrUsb => "LimeSDR USB",
                LimeSdrModel::LimeSdrMiniV2 => "LimeSDR Mini 2.0",
                LimeSdrModel::LimeNetMicro => "LimeNET Micro",
                LimeSdrModel::OtherFx3 => "Unknown LimeSDR model with FX3",
                LimeSdrModel::OtherFt601 => "Unknown LimeSDR model with FT601",
            }.to_string(),

            rx_ant: Some(
                cfg.rx_ant.clone().unwrap_or(
                    match model {
                        LimeSdrModel::LimeSdrUsb => "LNAL",
                        _ => "LNAW",
                    }
                    .to_string(),
                ),
            ),
            tx_ant: Some(
                cfg.tx_ant.clone().unwrap_or(
                    match model {
                        LimeSdrModel::LimeSdrUsb => "BAND1",
                        _ => "BAND2",
                    }
                    .to_string(),
                ),
            ),

            rx_gain: vec![
                ("LNA".to_string(), cfg.rx_gain_lna.unwrap_or(18.0)),
                ("TIA".to_string(), cfg.rx_gain_tia.unwrap_or(6.0)),
                ("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(10.0)),
            ],
            tx_gain: vec![
                ("PAD".to_string(), cfg.tx_gain_pad.unwrap_or(22.0)),
                ("IAMP".to_string(), cfg.tx_gain_iamp.unwrap_or(6.0)),
            ],

            // Minimum latency for BS/MS, maximum throughput for monitor
            rx_args: vec![("latency".to_string(), if mode == StackMode::Mon { "1" } else { "0" }.to_string())],
            tx_args: vec![("latency".to_string(), if mode == StackMode::Mon { "1" } else { "0" }.to_string())],

            ..Self::default(mode)
        }
    }

    fn settings_sxceiver(cfg: &Option<CfgSxCeiver>, mode: StackMode) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgSxCeiver::default() };

        // TODO: pass detected clock rate or list of supported sample rates
        // to get_settings and choose sample rate accordingly.
        let fs = 600e3;
        SdrSettings {
            name: "SXceiver".to_string(),
            fs,

            rx_ant: Some(cfg.rx_ant.clone().unwrap_or("RX".to_string())),
            tx_ant: Some(cfg.tx_ant.clone().unwrap_or("TX".to_string())),

            rx_gain: vec![
                ("LNA".to_string(), cfg.rx_gain_lna.unwrap_or(42.0)),
                ("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(16.0)),
            ],
            tx_gain: vec![
                ("DAC".to_string(), cfg.tx_gain_dac.unwrap_or(9.0)),
                ("MIXER".to_string(), cfg.tx_gain_mixer.unwrap_or(30.0)),
            ],

            rx_args: vec![("period".to_string(), block_size(fs).to_string())],
            tx_args: vec![("period".to_string(), block_size(fs).to_string())],

            ..Self::default(mode)
        }
    }

    fn settings_usrp(cfg: &Option<CfgUsrpB2xx>, mode: StackMode, model: UsrpModel) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgUsrpB2xx::default() };

        SdrSettings {
            name: match model {
                UsrpModel::B200 => "USRP B200",
                UsrpModel::B210 => "USRP B210",
                UsrpModel::Other => "Unknown USRP model",
            }.to_string(),

            rx_ant: Some(cfg.rx_ant.clone().unwrap_or("TX/RX".to_string())),
            tx_ant: Some(cfg.tx_ant.clone().unwrap_or("TX/RX".to_string())),

            rx_gain: vec![("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(50.0))],
            tx_gain: vec![("PGA".to_string(), cfg.tx_gain_pga.unwrap_or(35.0))],

            rx_args: vec![],
            tx_args: vec![],

            ..Self::default(mode)
        }
    }

    fn settings_pluto(cfg: &Option<CfgPluto>, mode: StackMode) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgPluto::default() };

        SdrSettings {
            name: "Pluto".to_string(),
            // get_hardware_time is apparently not implemented for pluto.
            use_get_hardware_time: false,

            // TODO: check if sample rate could be increased to 1024e3.
            // That would allow a power-of-two FFT size for lower CPU use.
            fs: 1e6,

            rx_ant: Some(cfg.rx_ant.clone().unwrap_or("A_BALANCED".to_string())),
            tx_ant: Some(cfg.tx_ant.clone().unwrap_or("A".to_string())),

            rx_gain: vec![("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(20.0))],
            tx_gain: vec![("PGA".to_string(), cfg.tx_gain_pga.unwrap_or(89.0))],

            rx_args: vec![],
            tx_args: vec![],

            dev_args: {
                let mut args = Vec::<(String, String)>::new();
                args.push((
                    "direct".to_string(),
                    cfg.direct.map_or("1", |v| if v { "1" } else { "0" }).to_string(),
                ));
                args.push(("timestamp_every".to_string(), cfg.timestamp_every.unwrap_or(1500).to_string()));
                if let Some(ref uri) = cfg.uri {
                    args.push(("uri".to_string(), uri.to_string()));
                }
                if let Some(loopback) = cfg.loopback {
                    args.push(("loopback".to_string(), (if loopback { "1" } else { "0" }).to_string()));
                }
                args
            },

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
