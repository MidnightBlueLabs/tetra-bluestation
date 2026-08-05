use soapysdr;
use tetra_config::bluestation::{SharedConfig, StackMode, sec_phy_soapy::CfgSoapySdr};

use tetra_pdus::phy::traits::rxtx_dev::RxTxDevError;

use super::dsp_types::*;
use super::soapy_settings;
use super::soapy_settings::{SdrSettings, SupportedDevice};
use super::soapy_time::{ticks_to_time_ns, time_ns_to_ticks};

type StreamType = ComplexSample;
const SOAPY_FREQ_OFFSET: f64 = 20000.0;

pub struct RxResult {
    /// Number of samples read
    pub len: usize,
    /// Sample counter for the first sample read
    pub count: SampleCount,
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
    prev_time_ns: i64,

    /// If false, timestamp of latest RX read is used to estimate
    /// current hardware time. This is used in case get_hardware_time
    /// is unacceptably slow or not supported.
    use_get_hardware_time: bool,
    /// If false, transmit continuously instead of timestamping each block.
    use_timed_tx: bool,

    dev: soapysdr::Device,
    /// Receive stream. None if receiving is disabled.
    rx: Option<soapysdr::RxStream<StreamType>>,
    /// MTU-sized native read buffer used by devices which cannot be drained
    /// reliably using the smaller blocks requested by the DSP.
    rx_staging: Vec<StreamType>,
    rx_staging_pos: usize,
    rx_staging_len: usize,
    rx_staging_count: SampleCount,
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
    pub fn new(cfg: &SharedConfig) -> Result<Self, soapysdr::Error> {
        let binding = cfg.config();
        let soapy_cfg = binding
            .phy_io
            .soapysdr
            .as_ref()
            .expect("SoapySdr config must be set for SoapySdr PhyIo");

        let mode = cfg.config().stack_mode;

        let (dev, sdr_settings) = open_device(&soapy_cfg, mode)?;

        let rx_ch = sdr_settings.rx_ch;
        let tx_ch = sdr_settings.tx_ch;

        // Get PPM corrected freqs
        let (dl_corrected, _) = soapy_cfg.dl_freq_corrected();
        let (ul_corrected, _) = soapy_cfg.ul_freq_corrected();

        let (rx_freq, tx_freq) = match mode {
            StackMode::Bs => (
                Some(ul_corrected - SOAPY_FREQ_OFFSET), // Offset RX center frequency from carrier frequency
                Some(dl_corrected),
            ),
            StackMode::Ms => (
                Some(dl_corrected - SOAPY_FREQ_OFFSET), // Offset RX center frequency from carrier frequency
                Some(ul_corrected),
            ),
            StackMode::Mon => {
                unimplemented!("Monitor mode not implemented yet");
            }
        };

        let rx_enabled = rx_freq.is_some();
        let tx_enabled = tx_freq.is_some();

        let mut rx_fs: f64 = 0.0;
        if rx_enabled {
            soapycheck!(
                "set RX sample rate",
                dev.set_sample_rate(soapysdr::Direction::Rx, rx_ch, sdr_settings.fs)
            );
            // Read the actual sample rate obtained and store it
            // to avoid having to read it again every time it is needed.
            rx_fs = soapycheck!("get RX sample rate", dev.sample_rate(soapysdr::Direction::Rx, rx_ch));
        }
        let mut tx_fs: f64 = 0.0;
        if tx_enabled {
            soapycheck!(
                "set TX sample rate",
                dev.set_sample_rate(soapysdr::Direction::Tx, tx_ch, sdr_settings.fs)
            );
            tx_fs = soapycheck!("get TX sample rate", dev.sample_rate(soapysdr::Direction::Tx, tx_ch));
        }

        if rx_enabled {
            // If rx_enabled is true, we already know rx_freq is not None,
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

        let mut rx_args = soapysdr::Args::new();
        for (key, value) in sdr_settings.rx_args {
            rx_args.set(key, value);
        }

        let mut tx_args = soapysdr::Args::new();
        for (key, value) in sdr_settings.tx_args {
            tx_args.set(key, value);
        }

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
        let rx_staging_capacity = if sdr_settings.stage_rx_to_mtu {
            if let Some(rx) = &rx {
                soapycheck!("get RX stream MTU", rx.mtu())
            } else {
                0
            }
        } else {
            0
        };

        Ok(Self {
            rx_ch,
            tx_ch,
            rx_fs,
            tx_fs,
            initial_time: None,
            rx_next_count: 0,
            prev_time_ns: -1,
            use_get_hardware_time: sdr_settings.use_get_hardware_time,
            use_timed_tx: sdr_settings.use_timed_tx,
            dev,
            rx,
            rx_staging: vec![StreamType::default(); rx_staging_capacity],
            rx_staging_pos: 0,
            rx_staging_len: 0,
            rx_staging_count: 0,
            tx,
        })
    }

    /// Read directly from SoapySDR and translate the native timestamp to the
    /// monotonically increasing sample counter used by the DSP.
    fn receive_native(&mut self, buffer: &mut [StreamType]) -> Result<RxResult, RxTxDevError> {
        if self.rx.is_none() {
            return Err(RxTxDevError::RxReadError);
        }

        loop {
            let read_result = {
                let rx = self.rx.as_mut().expect("RX stream was checked above");
                match rx.read(&mut [buffer], 1000000) {
                    Ok(len) => {
                        let time = rx.time_ns();
                        Ok((len, time))
                    }
                    Err(err) => Err(err),
                }
            };

            let (len, time) = match read_result {
                Ok(result) => result,
                Err(err) if err.code == soapysdr::ErrorCode::Overflow => {
                    tracing::warn!("SoapySDR RX overflow; resynchronizing from the next timestamp");
                    continue;
                }
                Err(err) => {
                    tracing::error!("SoapySDR RX read failed: {}", err);
                    return Err(RxTxDevError::RxReadError);
                }
            };

            if len == 0 {
                continue;
            }

            // rust-soapysdr does not expose whether a timestamp was available,
            // so infer it by checking whether the value changed.
            let timestamp_available = time != self.prev_time_ns;
            self.prev_time_ns = time;

            if self.initial_time.is_none() && timestamp_available {
                self.initial_time = Some(time - ticks_to_time_ns(self.rx_next_count, self.rx_fs));
                tracing::trace!("Set initial_time to {} ns", self.initial_time.unwrap());
            }

            // Re-compute total count from timestamp (gracefully handles lost samples).
            let mut count = if timestamp_available {
                time_ns_to_ticks(time - self.initial_time.unwrap(), self.rx_fs)
            } else {
                // Some drivers, particularly SoapyRemote, provide timestamps
                // only on some reads.
                self.rx_next_count
            };

            // Smooth tiny timestamp jitter (e.g. +/-1 sample) to keep counters monotonic.
            let delta_from_expected = count - self.rx_next_count;
            if delta_from_expected.abs() <= RX_TIMESTAMP_JITTER_TOLERANCE_SAMPLES {
                if delta_from_expected != 0 {
                    let initial_time = self.initial_time.unwrap() + ticks_to_time_ns(delta_from_expected, self.rx_fs);
                    self.initial_time = Some(initial_time);
                    tracing::debug!(
                        "RX timestamp jitter {} sample(s); re-anchoring initial_time by {} ns",
                        delta_from_expected,
                        ticks_to_time_ns(delta_from_expected, self.rx_fs)
                    );
                }
                count = self.rx_next_count;
            }

            self.rx_next_count = count + len as SampleCount;
            return Ok(RxResult { len, count });
        }
    }

    pub fn receive(&mut self, buffer: &mut [StreamType]) -> Result<RxResult, RxTxDevError> {
        if buffer.is_empty() {
            return Ok(RxResult {
                len: 0,
                count: self.rx_next_count,
            });
        }

        if self.rx_staging.is_empty() {
            return self.receive_native(buffer);
        }

        if self.rx_staging_pos == self.rx_staging_len {
            let mut staging = std::mem::take(&mut self.rx_staging);
            let result = self.receive_native(&mut staging);
            self.rx_staging = staging;
            let result = result?;
            self.rx_staging_pos = 0;
            self.rx_staging_len = result.len;
            self.rx_staging_count = result.count;
        }

        let count = self.rx_staging_count + self.rx_staging_pos as SampleCount;
        let len = buffer.len().min(self.rx_staging_len - self.rx_staging_pos);
        buffer[..len].copy_from_slice(&self.rx_staging[self.rx_staging_pos..self.rx_staging_pos + len]);
        self.rx_staging_pos += len;

        Ok(RxResult { len, count })
    }

    pub fn transmit(&mut self, buffer: &[StreamType], count: Option<SampleCount>) -> Result<(), RxTxDevError> {
        if let Some(tx) = &mut self.tx {
            if let Some(initial_time) = self.initial_time {
                tx.write_all(
                    &[buffer],
                    if self.use_timed_tx {
                        count.map(|count| initial_time + ticks_to_time_ns(count, self.tx_fs))
                    } else {
                        None
                    },
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

// Messy logic related to opening a device follows...

/// Struct to temporarily hold stuff related to opening and detecting a device
struct OpenedDevice {
    dev_args: soapysdr::Args,
    dev: soapysdr::Device,
    driver_key: String,
    hardware_key: String,
    detected_device: SupportedDevice,
    soapyremote_used: bool,
}

fn open_given_device(dev_args: soapysdr::Args) -> Result<OpenedDevice, soapysdr::Error> {
    let soapyremote_used = match dev_args.get("driver") {
        Some("remote") => true,
        _ => false,
    };
    tracing::info!("Trying to open a device with arguments: {}", dev_args);

    let dev_args_copy: soapysdr::Args = dev_args.iter().collect();
    let dev = match soapysdr::Device::new(dev_args_copy) {
        Ok(dev) => dev,
        Err(err) => {
            tracing::info!("Skipping a SoapySDR device because opening failed: {}", err);
            return Err(err);
        }
    };
    let driver_key = dev.driver_key().unwrap_or_default();
    let hardware_key = dev.hardware_key().unwrap_or_default();

    // Check whether the device is supported
    if let Some(detected_device) = SupportedDevice::detect(&driver_key, &hardware_key) {
        tracing::info!(
            "Found supported device with driver_key '{}' hardware_key '{}'",
            driver_key,
            hardware_key
        );
        Ok(OpenedDevice {
            dev_args,
            dev,
            driver_key,
            hardware_key,
            detected_device,
            soapyremote_used,
        })
    } else {
        tracing::info!(
            "Skipping unsupported device with driver_key '{}' hardware_key '{}'",
            driver_key,
            hardware_key
        );
        Err(soapysdr::Error {
            code: soapysdr::ErrorCode::NotSupported,
            message: "Unsupported device".to_string(),
        })
    }
}

/// Enumerate devices and find the first supported device
fn find_supported_device(filter_args: soapysdr::Args) -> Result<OpenedDevice, soapysdr::Error> {
    for dev_args in soapycheck!("Enumerate SoapySDR devices", soapysdr::enumerate(filter_args)) {
        //tracing::info!("Trying to open a device with arguments: {}", args_formatted);
        match open_given_device(dev_args) {
            Ok(opened_device) => return Ok(opened_device),
            Err(_) => {}
        }
    }
    return Err(soapysdr::Error {
        code: soapysdr::ErrorCode::NotSupported,
        message: "No supported devices found".to_string(),
    });
}

/// Open a given device if argument string is given,
/// automatically find the first supported device if not.
fn open_device(soapy_cfg: &CfgSoapySdr, mode: StackMode) -> Result<(soapysdr::Device, SdrSettings), soapysdr::Error> {
    let mut opened_device = if let Some(arg_string) = &soapy_cfg.device {
        open_given_device(arg_string.as_str().into())
    } else {
        find_supported_device(soapysdr::Args::new())
    }?;

    let mut sdr_settings = match SdrSettings::get_settings(&soapy_cfg, opened_device.detected_device, mode) {
        Ok(sdr_settings) => sdr_settings,
        Err(soapy_settings::Error::InvalidConfiguration) => {
            return Err(soapysdr::Error {
                code: soapysdr::ErrorCode::Other,
                message: "Invalid SDR device configuration".to_string(),
            });
        }
    };

    if opened_device.soapyremote_used {
        // Getting hardware time may be too slow over SoapyRemote
        tracing::info!("SoapyRemote detected, forcing use_get_hardware_time=false");
        sdr_settings.use_get_hardware_time = false;
    }

    tracing::info!("Using settings: {:?}", sdr_settings);

    // If additional driver arguments are needed, reopen the device with them
    if sdr_settings.dev_args.len() > 0 {
        // Append additional arguments from settings
        for (key, value) in &sdr_settings.dev_args {
            opened_device.dev_args.set(key.as_str(), value.as_str());
        }

        tracing::info!("Reopening device with additional arguments: {}", opened_device.dev_args);

        // Make sure device gets closed first. Not sure if needed.
        std::mem::drop(opened_device.dev);
        opened_device.dev = soapycheck!(
            "open SoapySDR device with additional arguments",
            soapysdr::Device::new(opened_device.dev_args)
        );
        // Make sure it is still the same device.
        // Unlikely to change, but who knows if a device got connected just in between,
        // or if the device broke from first opening attempt and something else got opened
        // because device arguments were not precise enough to guarantee a specific device.
        let new_driver_key = opened_device.dev.driver_key().unwrap_or_default();
        let new_hardware_key = opened_device.dev.hardware_key().unwrap_or_default();
        if new_driver_key != opened_device.driver_key || new_hardware_key != opened_device.hardware_key {
            tracing::info!(
                "Expected the same driver_key='{}' hardware_key='{}' after reopen, got driver_key='{}' hardware_key='{}'",
                opened_device.driver_key,
                opened_device.hardware_key,
                new_driver_key,
                new_hardware_key
            );
            return Err(soapysdr::Error {
                code: soapysdr::ErrorCode::Other,
                message: "Reopened a different device".to_string(),
            });
        }
    }

    Ok((opened_device.dev, sdr_settings))
}

#[cfg(test)]
mod hardware_tests {
    use std::time::{Duration, Instant};

    use super::{SoapyIo, StreamType};
    use tetra_config::bluestation::{SharedConfig, from_toml_str};

    #[test]
    #[ignore] // Requires exclusive access to bladeRF hardware and SoapyBladeRF.
    fn bladerf_sustains_full_duplex_streaming() {
        let serial = std::env::var("BLADERF_SERIAL").expect("set BLADERF_SERIAL to the bladeRF under test");
        let source = include_str!("../../../../../example_config/bladerf.toml").replace(
            "# device = \"driver=bladerf,serial=00000000000000000000000000000000\"",
            &format!("device = \"driver=bladerf,serial={}\"", serial),
        );
        let config = from_toml_str(&source).expect("bladeRF example should parse");
        let shared = SharedConfig::from_parts(config, None);
        let mut io = SoapyIo::new(&shared).expect("bladeRF should initialize");

        let mut rx_samples = vec![StreamType::default(); 768];
        let tx_samples = vec![StreamType::default(); 768];
        let started = Instant::now();
        let mut expected_count = None;
        let mut total_samples = 0usize;

        while started.elapsed() < Duration::from_secs(3) {
            let result = io.receive(&mut rx_samples).expect("bladeRF should return timestamped RX samples");
            if let Some(expected_count) = expected_count {
                assert_eq!(result.count, expected_count, "bladeRF RX timestamp discontinuity");
            }
            expected_count = Some(result.count + result.len as i64);
            io.transmit(&tx_samples[..result.len], Some(result.count))
                .expect("bladeRF should accept continuous TX samples");
            total_samples += result.len;
        }

        assert!(total_samples >= 1_400_000, "bladeRF streamed too few samples: {total_samples}");
    }
}
