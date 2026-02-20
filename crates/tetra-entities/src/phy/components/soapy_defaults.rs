#[derive(Clone, Debug)]
pub struct SdrSettings {
    /// Name used to print which SDR was detected
    pub name: String,
    /// Receive and transmit sample rate for TMO BS.
    pub fs_bs: f64,
    /// Receive and transmit sample rate for TMO monitor.
    /// The sample rate needs to be high enough to receive
    /// both downlink and uplink at the same time.
    pub fs_monitor: f64,
    /// Receive antenna
    pub rx_ant: Option<String>,
    /// Transmit antenna
    pub tx_ant: Option<String>,
    /// Receive gains
    pub rx_gain: Vec<(String, f64)>,
    /// Transmit gains
    pub tx_gain: Vec<(String, f64)>,
}

impl SdrSettings {
    /// Get default settings based on SDR type
    pub fn get_defaults(driver_key: &str, hardware_key: &str) -> Self {
        match (driver_key, hardware_key) {
            (_, "LimeSDR-USB") => Self::defaults_limesdr(),
            (_, "LimeSDR-Mini_v2") => Self::defaults_limesdr_mini_v2(),
            ("sx", _) => Self::defaults_sxceiver(),
            ("uhd", _) | ("b200", _) => Self::defaults_usrp_b2x0(),
            _ => Self::unknown(),
        }
    }

    fn unknown() -> Self {
        SdrSettings {
            name: "Unknown SDR device".to_string(),
            fs_bs: 512e3,
            fs_monitor: 16384e3,
            rx_ant: None,
            tx_ant: None,
            rx_gain: vec![],
            tx_gain: vec![],
        }
    }

    pub fn defaults_limesdr() -> Self {
        SdrSettings {
            name: "LimeSDR".to_string(),
            fs_bs: 512e3,
            fs_monitor: 16384e3,
            rx_ant: Some("LNAL".to_string()),
            tx_ant: Some("BAND1".to_string()),
            rx_gain: vec![("LNA".to_string(), 20.0), ("TIA".to_string(), 10.0), ("PGA".to_string(), 10.0)],
            tx_gain: vec![("PAD".to_string(), 52.0), ("IAMP".to_string(), 3.0)],
        }
    }

    pub fn defaults_limesdr_mini_v2() -> Self {
        SdrSettings {
            name: "LimeSDR Mini v2".to_string(),
            fs_bs: 512e3,
            fs_monitor: 16384e3,
            rx_ant: Some("LNAW".to_string()),
            tx_ant: Some("BAND2".to_string()),
            rx_gain: vec![("TIA".to_string(), 6.0), ("LNA".to_string(), 18.0), ("PGA".to_string(), 0.0)],
            tx_gain: vec![("PAD".to_string(), 30.0), ("IAMP".to_string(), 6.0)],
        }
    }

    pub fn defaults_sxceiver() -> Self {
        SdrSettings {
            name: "SXceiver".to_string(),
            fs_bs: 600e3,
            fs_monitor: 600e3, // monitoring is not really possible with SXceiver
            rx_ant: Some("RX".to_string()),
            tx_ant: Some("TX".to_string()),
            rx_gain: vec![("LNA".to_string(), 42.0), ("PGA".to_string(), 16.0)],
            tx_gain: vec![("DAC".to_string(), 9.0), ("MIXER".to_string(), 30.0)],
        }
    }

    pub fn defaults_usrp_b2x0() -> Self {
        SdrSettings {
            name: "USRP B200/B210".to_string(),
            fs_bs: 512e3,
            fs_monitor: 16384e3,
            rx_ant: Some("TX/RX".to_string()),
            tx_ant: Some("TX/RX".to_string()),
            rx_gain: vec![("PGA".to_string(), 50.0)],
            tx_gain: vec![("PGA".to_string(), 35.0)],
        }
    }
}
