use soapysdr;
use tetra_config::SharedConfig;

use tetra_pdus::phy::traits::rxtx_dev::RxTxDevError;

use super::dsp_types;
use super::dsp_types::*;
use super::soapy_defaults::SdrSettings;
use super::soapy_time::{ticks_to_time_ns, time_ns_to_ticks};

type StreamType = ComplexSample;

#[derive(Debug)]
pub enum Mode {
    Bs,
    Ms,
    Mon,
}

pub struct RxResult {
    /// Number of samples read
    pub len: usize,
    /// Sample counter for the first sample read
    pub count: dsp_types::SampleCount,
}

pub struct SoapyIo {
    rx_ch: usize,
    tx_ch: usize,
    rx_fs: f64,
    tx_fs: f64,
    /// Timestamp for the first sample read from SDR.
    /// This is subtracted from all following timestamps,
    /// so that sample counter startsB210 from 0 even if timestamp does not.
    initial_time: Option<i64>,
    rx_next_count: SampleCount,

    /// If false, timestamp of latest RX read is used to estimate
    /// current hardware time. This is used in case get_hardware_time
    /// is unacceptably slow, particularly with SoapyRemote.
    use_get_hardware_time: bool,

    dev: soapysdr::Device,
    /// Receive stream. None if receiving is disabled.
    rx: Option<soapysdr::RxStream<StreamType>>,
    /// Transmit stream. None if transmitting is disabled.
    tx: Option<soapysdr::TxStream<StreamType>>,
}

/// Soapy/Lime timestamps can occasionally jitter by a single sample.
/// Treat tiny deltas as contiguous to avoid triggering large block realignments downstream.
const RX_TIMESTAMP_JITTER_TOLERANCE_SAMPLES: SampleCount = 1;

/// It is annoying to repeat error handling so do that in a macro.
/// ? could be used but then it could not print which SoapySDR call failed.
macro_rules! soapycheck {
    ($text:literal, $soapysdr_call:expr) => {
        match $soapysdr_call {
            Ok(ret) => ret,
            Err(err) => {
                tracing::error!("SoapySDR: Failed to {}: {}", $text, err);
                return Err(err);
            }
        }
    };
}

impl SoapyIo {
    /// Get gain value from config or use default value from SdrSettings
    fn get_gain_or_default(gain_name: &str, cfg_val: Option<f64>, defaults: &SdrSettings) -> (String, f64) {
        if let Some(val) = cfg_val {
            (gain_name.to_string(), val)
        } else {
            defaults
                .rx_gain
                .iter()
                .find(|(name, _)| name == gain_name)
                .cloned()
                .unwrap_or_else(|| (gain_name.to_string(), 0.0))
        }
    }

    pub fn new(cfg: &SharedConfig, mode: Mode) -> Result<Self, soapysdr::Error> {
        let rx_ch = 0;
        let tx_ch = 0;
        let mut use_get_hardware_time = true;

        let binding = cfg.config();
        let soapy_cfg = binding
            .phy_io
            .soapysdr
            .as_ref()
            .expect("SoapySdr config must be set for SoapySdr PhyIo");
        let driver = soapy_cfg.io_cfg.get_soapy_driver_name();
        let dev_args_str = &[("driver", driver)];

        // Get PPM corrected freqs
        let (dl_corrected, _) = soapy_cfg.dl_freq_corrected();
        let (ul_corrected, _) = soapy_cfg.ul_freq_corrected();

        let (rx_freq, tx_freq) = match mode {
            Mode::Bs => (
                Some(ul_corrected - 20000.0), // Offset RX center frequency from carrier frequency
                Some(dl_corrected),
            ),
            Mode::Ms => (
                Some(dl_corrected - 20000.0), // Offset RX center frequency from carrier frequency
                Some(ul_corrected),
            ),
            Mode::Mon => {
                unimplemented!("Monitor mode not implemented yet");
            }
        };

        let mut dev_args = soapysdr::Args::new();
        for (key, value) in dev_args_str {
            dev_args.set(*key, *value);

            // get_hardware_time tends to be unacceptably slow
            // over SoapyRemote, so do not use it.
            // Maybe this is not a reliably way to detect use of SoapyRemote
            // in case SoapySDR selects it by default, but I do not know
            // a better way to detect it.
            if *key == "driver" && *value == "remote" {
                use_get_hardware_time = false;
            }
        }

        let dev = soapycheck!("open SoapySDR device", soapysdr::Device::new(dev_args));

        let rx_enabled = rx_freq.is_some();
        let tx_enabled = tx_freq.is_some();

        // Get default settings based on detected hardware
        let driver_key = dev.driver_key().unwrap_or_default();
        let hardware_key = dev.hardware_key().unwrap_or_default();
        let mut sdr_settings = SdrSettings::get_defaults(&driver_key, &hardware_key);

        // Apply user configuration overrides based on driver type
        let driver = soapy_cfg.io_cfg.get_soapy_driver_name();
        match driver {
            "uhd" => {
                if let Some(cfg) = &soapy_cfg.io_cfg.iocfg_usrpb2xx {
                    // Override antenna settings if specified
                    if let Some(ref ant) = cfg.rx_ant {
                        sdr_settings.rx_ant = Some(ant.clone());
                    }
                    if let Some(ref ant) = cfg.tx_ant {
                        sdr_settings.tx_ant = Some(ant.clone());
                    }

                    // Override gain settings
                    let mut rx_gains = Vec::new();
                    rx_gains.push(Self::get_gain_or_default("PGA", cfg.rx_gain_pga, &sdr_settings));
                    sdr_settings.rx_gain = rx_gains;

                    let mut tx_gains = Vec::new();
                    tx_gains.push(Self::get_gain_or_default("PGA", cfg.tx_gain_pga, &sdr_settings));
                    sdr_settings.tx_gain = tx_gains;
                }
            }
            "lime" => {
                if let Some(cfg) = &soapy_cfg.io_cfg.iocfg_limesdr {
                    // Override antenna settings if specified
                    if let Some(ref ant) = cfg.rx_ant {
                        sdr_settings.rx_ant = Some(ant.clone());
                    }
                    if let Some(ref ant) = cfg.tx_ant {
                        sdr_settings.tx_ant = Some(ant.clone());
                    }

                    // Override gain settings
                    let mut rx_gains = Vec::new();
                    rx_gains.push(Self::get_gain_or_default("LNA", cfg.rx_gain_lna, &sdr_settings));
                    rx_gains.push(Self::get_gain_or_default("TIA", cfg.rx_gain_tia, &sdr_settings));
                    rx_gains.push(Self::get_gain_or_default("PGA", cfg.rx_gain_pga, &sdr_settings));
                    sdr_settings.rx_gain = rx_gains;

                    let mut tx_gains = Vec::new();
                    tx_gains.push(Self::get_gain_or_default("PAD", cfg.tx_gain_pad, &sdr_settings));
                    tx_gains.push(Self::get_gain_or_default("IAMP", cfg.tx_gain_iamp, &sdr_settings));
                    sdr_settings.tx_gain = tx_gains;
                }
            }
            "sx" => {
                if let Some(cfg) = &soapy_cfg.io_cfg.iocfg_sxceiver {
                    // Override antenna settings if specified
                    if let Some(ref ant) = cfg.rx_ant {
                        sdr_settings.rx_ant = Some(ant.clone());
                    }
                    if let Some(ref ant) = cfg.tx_ant {
                        sdr_settings.tx_ant = Some(ant.clone());
                    }

                    // Override gain settings
                    let mut rx_gains = Vec::new();
                    rx_gains.push(Self::get_gain_or_default("LNA", cfg.rx_gain_lna, &sdr_settings));
                    rx_gains.push(Self::get_gain_or_default("PGA", cfg.rx_gain_pga, &sdr_settings));
                    sdr_settings.rx_gain = rx_gains;

                    let mut tx_gains = Vec::new();
                    tx_gains.push(Self::get_gain_or_default("DAC", cfg.tx_gain_dac, &sdr_settings));
                    tx_gains.push(Self::get_gain_or_default("MIXER", cfg.tx_gain_mixer, &sdr_settings));
                    sdr_settings.tx_gain = tx_gains;
                }
            }
            _ => {
                tracing::warn!("Unknown SoapySDR driver '{}', using default settings", driver);
            }
        }

        tracing::info!(
            "Got driver key '{}' hardware_key '{}', using settings for {}",
            driver_key,
            hardware_key,
            sdr_settings.name
        );

        let samp_rate = match mode {
            Mode::Bs | Mode::Ms => sdr_settings.fs_bs,
            Mode::Mon => sdr_settings.fs_monitor,
        };
        let mut rx_fs: f64 = 0.0;
        if rx_enabled {
            soapycheck!("set RX sample rate", dev.set_sample_rate(soapysdr::Direction::Rx, rx_ch, samp_rate));
            // Read the actual sample rate obtained and store it
            // to avoid having to read it again every time it is needed.
            rx_fs = soapycheck!("get RX sample rate", dev.sample_rate(soapysdr::Direction::Rx, rx_ch));
        }
        let mut tx_fs: f64 = 0.0;
        if tx_enabled {
            soapycheck!("set TX sample rate", dev.set_sample_rate(soapysdr::Direction::Tx, tx_ch, samp_rate));
            tx_fs = soapycheck!("get TX sample rate", dev.sample_rate(soapysdr::Direction::Tx, tx_ch));
        }

        if rx_enabled {
            // If rx_enabled is true, we already know sdr_rx_freq is not None,
            // so unwrap is fine here.
            soapycheck!(
                "set RX center frequency",
                dev.set_frequency(soapysdr::Direction::Rx, rx_ch, rx_freq.unwrap(), soapysdr::Args::new())
            );

            if let Some(ref ant) = sdr_settings.rx_ant {
                soapycheck!("set RX antenna", dev.set_antenna(soapysdr::Direction::Rx, rx_ch, ant.as_str()));
            }

            for (name, gain) in &sdr_settings.rx_gain {
                soapycheck!(
                    "set RX gain",
                    dev.set_gain_element(soapysdr::Direction::Rx, rx_ch, name.as_str(), *gain)
                );
            }
        }

        if tx_enabled {
            soapycheck!(
                "set TX center frequency",
                dev.set_frequency(soapysdr::Direction::Tx, tx_ch, tx_freq.unwrap(), soapysdr::Args::new())
            );

            if let Some(ref ant) = sdr_settings.tx_ant {
                soapycheck!("set TX antenna", dev.set_antenna(soapysdr::Direction::Tx, tx_ch, ant.as_str()));
            }

            for (name, gain) in &sdr_settings.tx_gain {
                soapycheck!(
                    "set TX gain",
                    dev.set_gain_element(soapysdr::Direction::Tx, tx_ch, name.as_str(), *gain)
                );
            }
        }

        // TODO: add stream arguments to SdrSettings.
        // Maybe they should be different for BS and monitor modes.
        // For example, the latency argument with LimeSDR should probably
        // be set for minimum latency for TMO BS
        // but for maximum throughput for TMO monitor.
        let mut rx_args = soapysdr::Args::new();
        let tx_args = soapysdr::Args::new();
        // hack to test the idea above, TODO properly
        match mode {
            Mode::Bs | Mode::Ms => {
                // Minimize latency
                rx_args.set("latency", "0");
            }
            Mode::Mon => {
                // Maximize throughput with high sample rates
                rx_args.set("latency", "1");
            }
        };

        let mut rx = if rx_enabled {
            Some(soapycheck!("setup RX stream", dev.rx_stream_args(&[rx_ch], rx_args)))
        } else {
            None
        };
        let mut tx = if tx_enabled {
            Some(soapycheck!("setup TX stream", dev.tx_stream_args(&[tx_ch], tx_args)))
        } else {
            None
        };
        if let Some(rx) = &mut rx {
            soapycheck!("activate RX stream", rx.activate(None));
        }
        if let Some(tx) = &mut tx {
            soapycheck!("activate TX stream", tx.activate(None));
        }
        Ok(Self {
            rx_ch,
            tx_ch,
            rx_fs,
            tx_fs,
            initial_time: None,
            rx_next_count: 0,
            use_get_hardware_time,
            dev,
            rx,
            tx,
        })
    }

    pub fn receive(&mut self, buffer: &mut [StreamType]) -> Result<RxResult, RxTxDevError> {
        if let Some(rx) = &mut self.rx {
            // RX is enabled
            match rx.read(&mut [buffer], 1000000) {
                Ok(len) => {
                    // Get timestamp, set initial time if not yet set
                    let time = rx.time_ns();
                    if self.initial_time.is_none() {
                        self.initial_time = Some(time - ticks_to_time_ns(self.rx_next_count, self.rx_fs));
                        tracing::trace!("Set initial_time to {} ns", self.initial_time.unwrap());
                    };

                    // Re-compute total count from timestamp (gracefully handles lost samples).
                    let mut count = time_ns_to_ticks(time - self.initial_time.unwrap(), self.rx_fs);

                    // Smooth tiny timestamp jitter (e.g. +/-1 sample) to keep counters monotonic
                    // This is known to happen for LimeSDR Mini v2 after some time
                    let delta_from_expected = count - self.rx_next_count;
                    if delta_from_expected.abs() <= RX_TIMESTAMP_JITTER_TOLERANCE_SAMPLES {
                        if delta_from_expected != 0 {
                            // Re-anchor phase so persistent +/-1 sample offset is corrected
                            let initial_time = self.initial_time.unwrap() + ticks_to_time_ns(delta_from_expected, self.rx_fs); // unwrap never fails
                            self.initial_time = Some(initial_time);
                            tracing::debug!(
                                "RX timestamp jitter {} sample(s); re-anchoring initial_time by {} ns",
                                delta_from_expected,
                                ticks_to_time_ns(delta_from_expected, self.rx_fs)
                            );
                        }
                        count = self.rx_next_count;
                    }

                    // Store expected sample count for the next sample to be read.
                    // This is used in case timestamp is missing.
                    self.rx_next_count = count + len as SampleCount;

                    Ok(RxResult { len, count })
                }
                Err(_) => Err(RxTxDevError::RxReadError),
            }
        } else {
            // RX is disabled
            Err(RxTxDevError::RxReadError)
        }
    }

    pub fn transmit(&mut self, buffer: &[StreamType], count: Option<SampleCount>) -> Result<(), RxTxDevError> {
        if let Some(tx) = &mut self.tx {
            if let Some(initial_time) = self.initial_time {
                tx.write_all(
                    &[buffer],
                    count.map(|count| initial_time + ticks_to_time_ns(count, self.tx_fs)),
                    false,
                    1000000,
                )
                .map_err(|_| RxTxDevError::RxReadError)
            } else {
                // initial_time is not available, so TX is not possible yet
                Err(RxTxDevError::RxReadError)
            }
        } else {
            // TX is disabled
            Err(RxTxDevError::RxReadError)
        }
    }

    pub fn current_time(&self) -> Result<i64, RxTxDevError> {
        self.dev.get_hardware_time(None).map_err(|_| RxTxDevError::RxReadError)
    }

    /// Current hardware time as RX sample count
    pub fn rx_current_count(&self) -> Result<SampleCount, RxTxDevError> {
        if !self.rx_enabled() {
            return Ok(0);
        }
        if self.use_get_hardware_time {
            Ok(time_ns_to_ticks(self.current_time()? - self.initial_time.unwrap_or(0), self.rx_fs))
        } else {
            Ok(self.rx_next_count - 1)
        }
    }

    /// Current hardware time as TX sample count
    pub fn tx_current_count(&self) -> Result<SampleCount, RxTxDevError> {
        if !self.tx_enabled() {
            return Ok(0);
        }
        if self.use_get_hardware_time {
            Ok(time_ns_to_ticks(self.current_time()? - self.initial_time.unwrap_or(0), self.tx_fs))
        } else {
            // Assumes equal RX and TX sample rates
            // and does not work if RX is disabled.
            // This is not a problem right now but could be fixed if needed.
            Ok(self.rx_next_count - 1)
        }
    }

    pub fn tx_possible(&self) -> bool {
        // initial_time is obtained from the first RX read (that includes a timestamp),
        // so prevent TX before it is available.
        self.tx_enabled() && self.initial_time.is_some()
    }

    pub fn rx_sample_rate(&self) -> f64 {
        self.rx_fs
    }

    pub fn tx_sample_rate(&self) -> f64 {
        self.tx_fs
    }

    pub fn rx_center_frequency(&self) -> Result<f64, soapysdr::Error> {
        self.dev.frequency(soapysdr::Direction::Rx, self.rx_ch)
    }

    pub fn tx_center_frequency(&self) -> Result<f64, soapysdr::Error> {
        self.dev.frequency(soapysdr::Direction::Tx, self.tx_ch)
    }

    pub fn rx_enabled(&self) -> bool {
        self.rx.is_some()
    }

    pub fn tx_enabled(&self) -> bool {
        self.tx.is_some()
    }
}
