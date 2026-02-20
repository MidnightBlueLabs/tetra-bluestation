//! Resampling, buffering and timestamp handling
//! between SDR device and modulator/demodulator code.

use rustfft;
use tetra_config::SharedConfig;
use tetra_config::StackMode;

use tetra_pdus::phy::traits::rxtx_dev::RxSlotBits;
use tetra_pdus::phy::traits::rxtx_dev::RxTxDev;
use tetra_pdus::phy::traits::rxtx_dev::RxTxDevError;
use tetra_pdus::phy::traits::rxtx_dev::TxSlotBits;

use crate::phy::components::soapy_dev;

use super::demodulator;
use super::dsp_types::*;
use super::fcfb;
use super::modulator;
use super::soapyio;

pub struct SdrConfig<'a> {
    /// SoapySDR device arguments
    pub dev_args: &'a [(&'a str, &'a str)],
    /// SDR RX center frequency
    pub rx_freq: Option<f64>,
    /// SDR TX center frequency
    pub tx_freq: Option<f64>,
}

#[derive(Default)]
pub struct PhyConfig<'a> {
    /// Downlink/uplink carrier frequency pairs to monitor.
    /// Uplink frequency can be set to None to monitor downlink only.
    pub monitor_frequencies: &'a [(f64, Option<f64>)],
    /// Downlink carrier frequencies for a BS.
    pub bs_dl_frequencies: &'a [f64],
    /// Uplink carrier frequencies for a BS.
    pub bs_ul_frequencies: &'a [f64],
}

pub struct RxTxDevSoapySdr {
    sdr: soapyio::SoapyIo,
    rx_dsp: Option<RxDsp>,
    tx_dsp: Option<TxDsp>,
}

type FftPlanner = rustfft::FftPlanner<RealSample>;

impl RxTxDevSoapySdr {
    // pub fn new(
    //     sdr_config: SdrConfig,
    //     phy_config: PhyConfig,
    // ) -> Self {
    //     let mut fft_planner = rustfft::FftPlanner::new();

    //     let mut sdr = soapyio::SoapyIo::new(
    //         sdr_config.dev_args,
    //         sdr_config.rx_freq,
    //         sdr_config.tx_freq,
    //         if !phy_config.monitor_frequencies.is_empty() {
    //             soapyio::Mode::Mon
    //         } else {
    //             soapyio::Mode::Bs
    //         },
    //     ).unwrap();

    //     Self {
    //         rx_dsp: if sdr.rx_enabled() {
    //             Some(RxDsp::new(&mut fft_planner, &mut sdr, &phy_config))
    //         } else { None },

    //         tx_dsp: if sdr.tx_enabled() {
    //             Some(TxDsp::new(&mut fft_planner, &mut sdr, &phy_config))
    //         } else { None },

    //         sdr,
    //     }
    // }

    pub fn new(cfg: &SharedConfig) -> Self {
        let mut fft_planner = rustfft::FftPlanner::new();

        // TODO FIXME we can remove the soapyio::mode enum and replace it by the globally used StackMode
        let mode = match cfg.config().stack_mode {
            StackMode::Bs => soapyio::Mode::Bs,
            StackMode::Ms => soapyio::Mode::Ms,
            StackMode::Mon => soapyio::Mode::Mon,
        };

        // TODO FIXME currently no MS and MON support in the below statement; need to fix
        let config_guard = cfg.config();
        let soapy_cfg = config_guard
            .as_ref()
            .phy_io
            .soapysdr
            .as_ref()
            .expect("Soapysdr config must be set for Soapysdr PhyIo");

        let (dl_corrected, dl_err) = soapy_cfg.dl_freq_corrected();
        let (ul_corrected, ul_err) = soapy_cfg.ul_freq_corrected();

        tracing::info!(
            "Freqs: DL / UL: {:.6} MHz / {:.6} MHz   PPM: {:.2} -> err {:.0} / {:.0} hz, adj {:.6} MHz / {:.6} MHz",
            soapy_cfg.dl_freq / 1e6,
            soapy_cfg.ul_freq / 1e6,
            soapy_cfg.ppm_err.unwrap_or(0.0),
            dl_err,
            ul_err,
            dl_corrected / 1e6,
            ul_corrected / 1e6
        );

        let phy_config = soapy_dev::PhyConfig {
            bs_dl_frequencies: &[dl_corrected],
            bs_ul_frequencies: &[ul_corrected],
            ..Default::default()
        };

        let mut sdr = soapyio::SoapyIo::new(cfg, mode).unwrap();

        Self {
            rx_dsp: if sdr.rx_enabled() {
                Some(RxDsp::new(&mut fft_planner, &mut sdr, &phy_config))
            } else {
                None
            },

            tx_dsp: if sdr.tx_enabled() {
                Some(TxDsp::new(&mut fft_planner, &mut sdr, &phy_config))
            } else {
                None
            },

            sdr,
        }
    }

    /// Process a block of received signal.
    /// Return true if processing can be continued,
    /// false if a slot has been demodulated and rxtx_timeslot should return.
    fn process_rx_block(&mut self) -> Result<bool, RxTxDevError> {
        if let Some(rx_dsp) = &mut self.rx_dsp {
            rx_dsp.process_block(&mut self.sdr)
        } else {
            Ok(false)
        }
    }

    /// Produce a block of transmit signal.
    /// Return true if processing can be continued,
    /// false if more data is needed
    /// or if it wants to wait before producing more.
    fn process_tx_block(&mut self, tx_slot: &[TxSlotBits]) -> Result<bool, RxTxDevError> {
        if let Some(tx_dsp) = &mut self.tx_dsp {
            if self.sdr.tx_possible() {
                tx_dsp.process_block(&mut self.sdr, self.rx_dsp.as_ref().map(|rx_dsp| rx_dsp.rx_block_count), tx_slot)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
}

impl RxTxDev for RxTxDevSoapySdr {
    fn rxtx_timeslot<'a>(
        &'a mut self,
        tx_slot: &[TxSlotBits],
        // TODO multiple demodulators
    ) -> Result<Vec<Option<RxSlotBits<'a>>>, RxTxDevError> {
        // First generate as much TX signal as possible at the moment.
        while self.process_tx_block(tx_slot)? {}

        while self.process_rx_block()? {
            // Continue producing TX signal if possible.
            while self.process_tx_block(tx_slot)? {}
        }

        if let Some(rx_dsp) = &mut self.rx_dsp {
            Ok(rx_dsp.take_slot_bits())
        } else {
            Ok(Default::default())
        }
    }
}

struct RxDsp {
    rx_fcfb: fcfb::AnalysisInputProcessor,

    rx_block_size: fcfb::InputBlockSize,
    rx_buffer: Vec<ComplexSample>,
    /// How much of rx_buffer has been filled
    rx_buffer_i: usize,
    rx_block_count: fcfb::BlockCount,

    monitors: Vec<MonitorDlUlPair>,
    ul_demodulators: Vec<DemodulatorChannel>,
}

impl RxDsp {
    fn new(fft_planner: &mut FftPlanner, sdr: &mut soapyio::SoapyIo, phy_config: &PhyConfig) -> Self {
        let sdr_sample_rate = sdr.rx_sample_rate();
        let rx_fcfb_params = fcfb::AnalysisInputParameters {
            // Use a bin spacing of 500 Hz.
            // This is a submultiple of the 72 kHz modem sample rate
            // and allows tuning in steps of 500 Hz.
            fft_size: (sdr_sample_rate / 500.0).round() as usize,
            center_frequency: sdr.rx_center_frequency().unwrap(),
            sample_rate: sdr_sample_rate,
            overlap: fcfb::Overlap::O1_4,
        };

        let fcfb = fcfb::AnalysisInputProcessor::new(fft_planner, rx_fcfb_params);
        let rx_block_size = fcfb.input_block_size();

        Self {
            rx_block_size,
            rx_buffer: vec![num::zero(); rx_block_size.overlap + rx_block_size.new],
            rx_buffer_i: 0,
            rx_fcfb: fcfb,
            rx_block_count: 0,

            monitors: phy_config
                .monitor_frequencies
                .iter()
                .map(|(dl_freq, ul_freq)| MonitorDlUlPair {
                    dl: DemodulatorChannel::new(fft_planner, rx_fcfb_params, *dl_freq, demodulator::Mode::DlUnsynchronized),
                    ul: ul_freq
                        .as_ref()
                        .map(|ul_freq| DemodulatorChannel::new(fft_planner, rx_fcfb_params, *ul_freq, demodulator::Mode::Idle)),
                })
                .collect(),

            ul_demodulators: phy_config
                .bs_ul_frequencies
                .iter()
                .map(|ul_freq| DemodulatorChannel::new(fft_planner, rx_fcfb_params, *ul_freq, demodulator::Mode::Ul))
                .collect(),
        }
    }

    fn process_block(&mut self, sdr: &mut soapyio::SoapyIo) -> Result<bool, RxTxDevError> {
        self.receive_block(sdr)?;

        let fcfb_result = self.rx_fcfb.process(&self.rx_buffer[..], self.rx_block_count);

        let mut continue_processing = true;

        for pair in self.monitors.iter_mut() {
            let continue_dl = pair.dl.process(fcfb_result, self.rx_block_count);
            if let Some(ul) = &mut pair.ul {
                ul.demodulator.sync_to_demodulator(&pair.dl.demodulator);
                continue_processing = ul.process(fcfb_result, self.rx_block_count) && continue_processing;
            } else {
                continue_processing = continue_dl && continue_processing;
            }
        }

        for demod in self.ul_demodulators.iter_mut() {
            continue_processing = demod.process(fcfb_result, self.rx_block_count) && continue_processing;
        }

        Ok(continue_processing)
    }

    fn receive_block(&mut self, sdr: &mut soapyio::SoapyIo) -> Result<(), RxTxDevError> {
        self.rx_block_count += 1;

        // Copy overlapping part from previous block to the beginning
        self.rx_buffer
            .copy_within(self.rx_block_size.new..self.rx_block_size.new + self.rx_block_size.overlap, 0);
        self.rx_buffer_i = self.rx_block_size.overlap;

        loop {
            let result = sdr.receive(&mut self.rx_buffer[self.rx_buffer_i..])?;

            let block_size = self.rx_block_size.new as SampleCount;
            let expected_count = self.rx_block_count as SampleCount * block_size + self.rx_buffer_i as SampleCount;
            let samples_lost = result.count - expected_count;
            if samples_lost != 0 {
                // Samples have been lost.
                // Mark RX buffer as empty and skip the right number of samples
                // to receive the next full processing block in the next iteration.

                // Expected sample count for the next read,
                // assuming no more samples are lost.
                let next_count = result.count + result.len as SampleCount;
                // div_euclid always rounds down (towards negative numbers),
                // so use it with negations to round up to the next block.
                let next_possible_block = -next_count.div_euclid(-block_size) + 1;
                let next_block_beginning = next_possible_block * block_size;

                let mut samples_to_skip = next_block_beginning - next_count;

                tracing::warn!(
                    "Lost {} samples, skipping {} more samples and {} processing blocks",
                    samples_lost,
                    samples_to_skip,
                    next_possible_block - self.rx_block_count
                );

                self.rx_block_count = next_possible_block;
                self.rx_buffer_i = 0;

                // Repeat reads until the correct number of samples has been skipped.
                while samples_to_skip > 0 {
                    let result = sdr.receive(&mut self.rx_buffer[0..samples_to_skip as usize])?;
                    samples_to_skip -= result.len as SampleCount;
                }
            } else {
                self.rx_buffer_i += result.len;
                if self.rx_buffer_i == self.rx_buffer.len() {
                    // tracing::trace!("Received processing block {} ({} samples in SDR buffer)",
                    //     self.rx_block_count,
                    //     // incorrect if time is not available but does not really matter
                    //     sdr.rx_current_count().unwrap_or(0) - (result.count + result.len as SampleCount - 1),
                    // );
                    return Ok(());
                }
            }
        }
    }

    fn take_slot_bits<'a>(&'a mut self) -> Vec<Option<RxSlotBits<'a>>> {
        // TODO: avoid dynamic allocation here?
        let mut slot_bits = Vec::with_capacity(2 * self.monitors.len() + self.ul_demodulators.len());

        for pair in self.monitors.iter_mut() {
            slot_bits.push(pair.dl.demodulator.take_demodulated_slot());
            slot_bits.push(if let Some(ul) = &mut pair.ul {
                ul.demodulator.take_demodulated_slot()
            } else {
                None
            });
        }

        for demod in self.ul_demodulators.iter_mut() {
            slot_bits.push(demod.demodulator.take_demodulated_slot());
        }

        slot_bits
    }
}

struct TxDsp {
    fcfb: fcfb::SynthesisOutputProcessor,
    block_count: fcfb::BlockCount,
    initial_time: i64,
    modulators: Vec<ModulatorChannel>,
}

impl TxDsp {
    fn new(fft_planner: &mut FftPlanner, sdr: &mut soapyio::SoapyIo, phy_config: &PhyConfig) -> Self {
        let sdr_sample_rate = sdr.tx_sample_rate();
        let fcfb_params = fcfb::SynthesisOutputParameters {
            ifft_size: (sdr_sample_rate / 500.0).round() as usize,
            center_frequency: sdr.tx_center_frequency().unwrap(),
            sample_rate: sdr_sample_rate,
            overlap: fcfb::Overlap::O1_4,
        };

        let fcfb = fcfb::SynthesisOutputProcessor::new(fft_planner, fcfb_params);

        let mut modulators = Vec::<ModulatorChannel>::new();
        for dl_freq in phy_config.bs_dl_frequencies {
            modulators.push(ModulatorChannel::new(fft_planner, fcfb_params, *dl_freq, modulator::Mode::Dl));
        }

        Self {
            fcfb,
            block_count: 0,
            initial_time: 0, // TODO: get it from RX
            modulators,
        }
    }

    fn process_block(
        &mut self,
        sdr: &mut soapyio::SoapyIo,
        latest_rx_block: Option<fcfb::BlockCount>,
        tx_slot: &[TxSlotBits],
    ) -> Result<bool, RxTxDevError> {
        let current_sample = sdr.tx_current_count()?;
        // Current time as block count
        let current_block = current_sample.div_euclid(self.fcfb.output_block_size() as SampleCount);

        let d = self.block_count - current_block;
        // Skip TX blocks in the past or in too near future
        let dmin = 2; // how many blocks in future minimum
        if d < dmin {
            let new_block_count = current_block + dmin;
            tracing::warn!(
                "Too late to produce TX block {}, skipping {} TX blocks",
                self.block_count,
                new_block_count - self.block_count
            );
            self.block_count = new_block_count;
        }
        // Limit how far into future TX blocks are generated
        let dmax = 60;
        if d > dmax {
            return Ok(false);
        }
        // Also limit how far from the latest RX block TX blocks are generated.
        // This prevents TX from ending up in an infinite loop
        // which does not give a chance for RX signal to get processed.
        //
        // This is not strictly necessary right now but might become useful
        // with different modulator operating modes in the future.
        //
        // Maybe the limit using hardware time above is redundant.
        if let Some(latest_rx_block) = latest_rx_block {
            let d_rx = self.block_count - latest_rx_block;
            if d_rx > dmax {
                return Ok(false);
            }
        }

        for (modulator, tx_slot) in self.modulators.iter_mut().zip(tx_slot) {
            if !modulator.process(&mut self.fcfb, self.block_count, tx_slot) {
                return Ok(false);
            }
        }

        let tx_signal = self.fcfb.process();

        // TODO: compensate for delay of SDR
        let sdr_sample_count = tx_signal.len() as SampleCount * self.block_count;

        // Increment block count before calling sdr.transmit with ?,
        // so we do not end up producing the same block again even if transmit fails.
        self.block_count += 1;

        sdr.transmit(tx_signal, Some(sdr_sample_count))?;

        // tracing::trace!("Produced transmit block {} ({} samples in future)",
        //     self.block_count - 1,
        //     sdr_sample_count - sdr.tx_current_count().unwrap_or(0),
        // );

        Ok(true)
    }
}

struct DemodulatorChannel {
    downconverter: fcfb::AnalysisOutputProcessor,
    demodulator: demodulator::Demodulator,
}

impl DemodulatorChannel {
    fn new(
        fft_planner: &mut FftPlanner,
        analysis_in_params: fcfb::AnalysisInputParameters,
        frequency: f64,
        mode: demodulator::Mode,
    ) -> Self {
        Self {
            downconverter: fcfb::AnalysisOutputProcessor::new_with_frequency(
                fft_planner,
                analysis_in_params,
                demodulator::SAMPLE_RATE,
                frequency,
                Some(25000.0),
            ),
            demodulator: demodulator::Demodulator::new(mode),
        }
    }

    /// Return true if processing should be continued,
    /// false if a new demodulated slot is available.
    fn process(&mut self, fcfb_result: &fcfb::AnalysisIntermediateResult, block_count: fcfb::BlockCount) -> bool {
        let samples = self.downconverter.process(fcfb_result);
        for (i, sample) in samples.iter().enumerate() {
            // TODO: include delay of FCFB in sample count
            self.demodulator.sample(
                *sample,
                block_count as SampleCount * samples.len() as SampleCount + i as SampleCount,
            );
        }
        !self.demodulator.demodulated_slot_available()
    }
}

struct ModulatorChannel {
    upconverter: fcfb::SynthesisInputProcessor,
    modulator: modulator::Modulator,
    /// Buffer for modulated signal at modulator sample rate.
    buffer: fcfb::InputBuffer,
    /// How much of buffer is filled
    buffer_i: usize,
}

impl ModulatorChannel {
    fn new(
        fft_planner: &mut FftPlanner,
        synthesis_out_params: fcfb::SynthesisOutputParameters,
        frequency: f64,
        mode: modulator::Mode,
    ) -> Self {
        let upconverter = fcfb::SynthesisInputProcessor::new_with_frequency(
            fft_planner,
            synthesis_out_params,
            modulator::SAMPLE_RATE,
            frequency,
            Some(25000.0),
        );
        Self {
            buffer: upconverter.make_input_buffer(),
            buffer_i: 0,
            upconverter,
            modulator: modulator::Modulator::new(mode),
        }
    }

    fn process(&mut self, fcfb: &mut fcfb::SynthesisOutputProcessor, block_count: fcfb::BlockCount, tx_slot: &TxSlotBits) -> bool {
        let buf = self.buffer.buffer_in();
        while self.buffer_i < buf.len() {
            // TODO: include delay of FCFB in sample count
            match self.modulator.sample(
                block_count as SampleCount * buf.len() as SampleCount + self.buffer_i as SampleCount,
                tx_slot,
            ) {
                Ok(sample) => {
                    buf[self.buffer_i] = sample;
                    self.buffer_i += 1;
                }
                Err(modulator::Error::NeedMoreData) => {
                    return false;
                }
            }
        }
        fcfb.add(self.upconverter.process(self.buffer.buffer(), block_count));

        let _ = self.buffer.prepare_for_new_samples();
        self.buffer_i = 0;
        true
    }
}

struct MonitorDlUlPair {
    dl: DemodulatorChannel,
    ul: Option<DemodulatorChannel>,
}
