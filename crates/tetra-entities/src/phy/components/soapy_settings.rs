//! Device-specific SoapySDR settings

use tetra_config::bluestation::{StackMode, sec_phy_soapy::*};

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
}

/// Get device arguments based on IO configuration.
///
/// This is separate from SdrSettings because device arguments
/// must be known before opening the device,
/// whereas SdrSettings may depend on information
/// that is obtained after the device has been opened.
pub fn get_device_arguments(io_cfg: &SoapySdrIoCfg, _mode: StackMode) -> Vec<(String, String)> {
    let mut args = Vec::<(String, String)>::new();

    let driver = io_cfg.get_soapy_driver_name();
    args.push(("driver".to_string(), driver.to_string()));

    // Additional device arguments for devices that need them
    match driver {
        "plutosdr" => {
            let cfg: &Option<CfgPluto> = &io_cfg.iocfg_pluto;
            // If cfg is None, use default which sets all optional fields to None.
            let cfg_pluto = if let Some(cfg) = cfg { &cfg } else { &CfgPluto::default() };

            args.push((
                "direct".to_string(),
                cfg_pluto.direct.map_or("1", |v| if v { "1" } else { "0" }).to_string(),
            ));
            args.push(("timestamp_every".to_string(), cfg_pluto.timestamp_every.unwrap_or(1500).to_string()));
            if let Some(ref uri) = cfg_pluto.uri {
                args.push(("uri".to_string(), uri.to_string()));
            }
            if let Some(loopback) = cfg_pluto.loopback {
                args.push(("loopback".to_string(), (if loopback { "1" } else { "0" }).to_string()));
            }
        }
        _ => {}
    }

    args
}

impl SdrSettings {
    /// Get settings based on SDR type
    pub fn get_settings(io_cfg: &SoapySdrIoCfg, driver_key: &str, hardware_key: &str, mode: StackMode) -> Self {
        match (driver_key, hardware_key) {
            ("FX3", "LimeSDR-USB") => Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, LimeSdrModel::LimeSdrUsb),
            ("FX3", "LimeSDR-Mini_v2") => Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, LimeSdrModel::LimeSdrMiniV2),
            ("FX3", _) => Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, LimeSdrModel::OtherFx3),

            // TODO: remove one of these once we know whether LimeSDR-Mini_v2 reports FX3 or FT601
            ("FT601", "LimeSDR-Mini_v2") => Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, LimeSdrModel::LimeSdrMiniV2),
            ("FT601", "LimeNET-Micro") => Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, LimeSdrModel::LimeNetMicro),
            ("FT601", _) => Self::settings_limesdr(&io_cfg.iocfg_limesdr, mode, LimeSdrModel::OtherFt601),

            ("sx", _) => Self::settings_sxceiver(&io_cfg.iocfg_sxceiver),

            // Is b200 actually reported as the driver in some cases?
            // Or is it a mistake and could be removed from here?
            // Or is b200 supposed to be the hardware key, like ("uhd", "b200")?
            // Should we add similar logic as LimeSDR for different USRP models?
            ("uhd", _) | ("b200", _) => Self::settings_usrp_b2x0(&io_cfg.iocfg_usrpb2xx, mode),

            ("PlutoSDR", _) => Self::settings_pluto(&io_cfg.iocfg_pluto, mode),

            _ => Self::unknown(mode),
        }
    }

    fn unknown(mode: StackMode) -> Self {
        SdrSettings {
            name: "Unknown SDR device".to_string(),
            use_get_hardware_time: true,
            fs: if mode == StackMode::Mon { 16384e3 } else { 512e3 },
            rx_ant: None,
            tx_ant: None,
            rx_gain: vec![],
            tx_gain: vec![],
            rx_args: vec![],
            tx_args: vec![],
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
            use_get_hardware_time: true,
            fs: if mode == StackMode::Mon { 16384e3 } else { 512e3 },

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
                ("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(0.0)),
            ],
            tx_gain: vec![
                ("PAD".to_string(), cfg.tx_gain_pad.unwrap_or(30.0)),
                ("IAMP".to_string(), cfg.tx_gain_iamp.unwrap_or(6.0)),
            ],

            // Minimum latency for BS/MS, maximum throughput for monitor
            rx_args: vec![("latency".to_string(), if mode == StackMode::Mon { "1" } else { "0" }.to_string())],
            tx_args: vec![("latency".to_string(), if mode == StackMode::Mon { "1" } else { "0" }.to_string())],
        }
    }

    fn settings_sxceiver(cfg: &Option<CfgSxCeiver>) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgSxCeiver::default() };

        let fs = 600e3;
        SdrSettings {
            name: "SXceiver".to_string(),
            use_get_hardware_time: true,
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
        }
    }

    fn settings_usrp_b2x0(cfg: &Option<CfgUsrpB2xx>, mode: StackMode) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgUsrpB2xx::default() };

        SdrSettings {
            name: "USRP B200/B210".to_string(),
            use_get_hardware_time: true,
            fs: if mode == StackMode::Mon { 16384e3 } else { 512e3 },

            rx_ant: Some(cfg.rx_ant.clone().unwrap_or("TX/RX".to_string())),
            tx_ant: Some(cfg.tx_ant.clone().unwrap_or("TX/RX".to_string())),

            rx_gain: vec![("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(50.0))],
            tx_gain: vec![("PGA".to_string(), cfg.tx_gain_pga.unwrap_or(35.0))],

            rx_args: vec![],
            tx_args: vec![],
        }
    }

    fn settings_pluto(cfg: &Option<CfgPluto>, mode: StackMode) -> Self {
        // If cfg is None, use default which sets all optional fields to None.
        let cfg = if let Some(cfg) = cfg { &cfg } else { &CfgPluto::default() };

        SdrSettings {
            name: "Pluto".to_string(),
            use_get_hardware_time: false,
            fs: if mode == StackMode::Mon { 1e6 } else { 1e6 },

            rx_ant: Some(cfg.rx_ant.clone().unwrap_or("A_BALANCED".to_string())),
            tx_ant: Some(cfg.tx_ant.clone().unwrap_or("A".to_string())),

            rx_gain: vec![("PGA".to_string(), cfg.rx_gain_pga.unwrap_or(20.0))],
            tx_gain: vec![("PGA".to_string(), cfg.tx_gain_pga.unwrap_or(89.0))],

            rx_args: vec![],
            tx_args: vec![],
        }
    }
}

#[derive(Debug, PartialEq)]
enum LimeSdrModel {
    LimeSdrUsb,
    LimeSdrMiniV2,
    LimeNetMicro,
    /// Other LimeSDR models with FX3 driver
    OtherFx3,
    /// Other LimeSDR models with FT601 driver
    OtherFt601,
}

/// Get processing block size in samples for a given sample rate.
/// This can be used to optimize performance for some SDRs.
pub fn block_size(fs: f64) -> usize {
    // With current FCFB parameters processing blocks are 1.5 ms long.
    // It is a bit bug prone to have it here in case
    // FCFB parameters are changed, but it makes things simpler for now.
    (fs * 1.5e-3).round() as usize
}
