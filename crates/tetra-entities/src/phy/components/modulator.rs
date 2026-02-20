use tetra_core::TdmaTime;

use tetra_pdus::phy::traits::rxtx_dev::TxSlotBits;

use crate::phy::components::dsp_types::*;
use crate::phy::components::fir;
use crate::phy::components::modem_common::*;

/// Samples per symbol
const SPS: SampleCount = 4;

/// Samples per slot
const SAMPLES_SLOT: SampleCount = SPS * 255;

/// Output sample rate
pub const SAMPLE_RATE: f64 = 18000.0 * SPS as f64;

#[derive(PartialEq)]
pub enum Mode {
    /// Downlink modulation.
    Dl,
}

pub struct Modulator {
    mode: Mode,
    /// Sample counter value at the beginning of hyperframe number 0
    reference_time: SampleCount,
    /// Pulse shaping filter
    filter: fir::FirComplexSym,
    dqpsk: DqpskMapper,
}

pub enum Error {
    /// Modulator needs data for another slot
    /// before it can continue producing TX signal.
    NeedMoreData,
}

impl Modulator {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            reference_time: 0,
            filter: fir::FirComplexSym::new(CHANNEL_FILTER_TAPS.len()),
            dqpsk: DqpskMapper::new(),
        }
    }

    /// Produce one output sample.
    pub fn sample(&mut self, sample_counter: SampleCount, tx_slot: &TxSlotBits) -> Result<ComplexSample, Error> {
        // Compensate for delay of pulse shaping filter in sample count
        let sample_counter = sample_counter + CHANNEL_FILTER_TAPS.len() as SampleCount;

        // Sample counter at beginning of current slot.
        // TODO: adjust self.reference_time when hyperframe number wraps to 0.
        // Now it breaks after 46 days.
        // This could also be further optimized by computing and storing it
        // only when a new slot becomes available.
        let slot_begin = self.reference_time + TdmaTime::to_int(tx_slot.time) as SampleCount * SAMPLES_SLOT;

        let mut sample = ComplexSample::ZERO;
        match self.mode {
            Mode::Dl => {
                let sample_in_slot = sample_counter - slot_begin;
                if sample_in_slot < 0 {
                    // Slot is in the future.
                    // Transmit silence until we reach the slot.
                } else if sample_in_slot >= SAMPLES_SLOT {
                    // Slot is in the past, so it has already been transmitted.
                    // Return and wait for data for the next slot to be available.
                    return Err(Error::NeedMoreData);
                } else if let Some(bits) = tx_slot.slot {
                    if sample_in_slot % SPS == 0 {
                        let symbol_i = (sample_in_slot / SPS) as usize;
                        sample = self.dqpsk.symbol(bits[symbol_i * 2] != 0, bits[symbol_i * 2 + 1] != 0);
                    }
                }
            }
        }
        Ok(self.filter.sample(&CHANNEL_FILTER_TAPS, sample))
    }
}

struct DqpskMapper {
    pub phase: i8,
}

impl DqpskMapper {
    pub fn new() -> Self {
        Self { phase: 0 }
    }

    #[allow(dead_code)]
    pub fn reset_phase(&mut self) {
        self.phase = 0;
    }

    pub fn symbol(&mut self, bit0: bool, bit1: bool) -> ComplexSample {
        self.phase = (self.phase
            + match (bit0, bit1) {
                (true, true) => -3,
                (true, false) => -1,
                (false, false) => 1,
                (false, true) => 3,
            })
            & 7;
        // Look-up table to map phase (in multiples of pi/4)
        // to constellation points. Generated in Python with:
        // import numpy as np
        // print(",\n".join("ComplexSample{ re: %9.6f, im: %9.6f }" % (v.real, v.imag) for v in np.exp(1j*np.linspace(0, np.pi*2, 8, endpoint=False))))
        const CONSTELLATION: [ComplexSample; 8] = [
            ComplexSample {
                re: 1.000000,
                im: 0.000000,
            },
            ComplexSample {
                re: 0.707107,
                im: 0.707107,
            },
            ComplexSample {
                re: 0.000000,
                im: 1.000000,
            },
            ComplexSample {
                re: -0.707107,
                im: 0.707107,
            },
            ComplexSample {
                re: -1.000000,
                im: 0.000000,
            },
            ComplexSample {
                re: -0.707107,
                im: -0.707107,
            },
            ComplexSample {
                re: -0.000000,
                im: -1.000000,
            },
            ComplexSample {
                re: 0.707107,
                im: -0.707107,
            },
        ];
        CONSTELLATION[self.phase as usize]
    }
}
