use num;
use num::complex::ComplexFloat;

use tetra_core::TdmaTime;
use tetra_core::TrainingSequence;
use tetra_pdus::phy::traits::rxtx_dev::RxBurstBits;
use tetra_pdus::phy::traits::rxtx_dev::RxSlotBits;

use crate::phy::components::train_consts;

use super::dsp_types::*;
use super::fir;
use super::history;
use super::modem_common::*;

/// Samples per symbol
pub const SPS: usize = 4;

/// Samples per symbol as SampleCount
pub const SAMPLES_SYMBOL: SampleCount = SPS as SampleCount;

/// Samples per slot
const SAMPLES_SLOT: SampleCount = SAMPLES_SYMBOL * 255;

/// Input sample rate
pub const SAMPLE_RATE: f64 = 18000.0 * SPS as f64;

#[derive(Copy, Clone, PartialEq)]
pub enum Mode {
    /// Do nothing.
    /// This is used in uplink monitor when the corresponding
    /// downlink demodulator is unsynchronized.
    Idle,
    /// Downlink demodulation before slot timing is known.
    /// The demodulator looks at overlapping blocks of samples
    /// and tries to find a synchronization training sequence
    /// anywhere within a block.
    DlUnsynchronized,
    /// Downlink demodulation with known slot timing.
    Dl,
    /// Uplink demodulator.
    Ul,
}

pub struct Demodulator {
    mode: Mode,

    /// Sample counter value at the beginning of hyperframe number 0
    reference_time: SampleCount,

    /// Sample counter value when the current slot is ready to be demodulated from past samples.
    slot_ready_time: SampleCount,

    /// Slot counter
    current_slot: TdmaTime,

    next_input_sample_count: SampleCount,

    /// Used to track timing in downlink demodulation.
    averaged_timing_error: RealSample,

    slots_since_last_valid_burst: u32,

    /// Timeslot of latest demodulated slot
    demodulated_slot_time: TdmaTime,
    demodulated_slot_available: bool,

    full_slot: SlotBurstFinder,
    subslot1: SlotBurstFinder,
    subslot2: SlotBurstFinder,

    matched_filter: fir::FirComplexSym,

    past_samples: history::History<ComplexSample, { SPS * 512 }>,
    /// Absolute values of past samples,
    /// used for symbol timing estimation.
    past_samples_abs: history::History<RealSample, { SPS * 512 }>,
}

impl Demodulator {
    pub fn new(initial_mode: Mode) -> Self {
        let mut self_ = Self {
            mode: initial_mode,
            reference_time: 0,
            slot_ready_time: 0, // will be set by set_slot_ready_time
            current_slot: TdmaTime::default(),

            next_input_sample_count: 0,
            averaged_timing_error: 0.0,
            slots_since_last_valid_burst: 0,

            demodulated_slot_time: Default::default(),
            demodulated_slot_available: false,

            full_slot: SlotBurstFinder::new(),
            subslot1: SlotBurstFinder::new(),
            subslot2: SlotBurstFinder::new(),

            matched_filter: fir::FirComplexSym::new(CHANNEL_FILTER_TAPS.len()),
            past_samples: history::History::new(num::zero()),
            past_samples_abs: history::History::new(num::zero()),
        };

        self_.set_slot_ready_time();
        self_
    }

    fn add_slots(&mut self, slots: i32) {
        self.set_slot(self.current_slot.add_timeslots(slots));
    }

    fn set_slot(&mut self, slot: TdmaTime) {
        self.current_slot = slot;
        self.set_slot_ready_time();
    }

    fn set_slot_ready_time(&mut self) {
        let slot_begin_time = self.reference_time + self.current_slot.to_int() as SampleCount * SAMPLES_SLOT;
        self.slot_ready_time = slot_begin_time + Self::slot_ready_from_begin(self.mode);
    }

    /// Number of samples to beginning of a slot to the point where it is ready to be demodulated
    fn slot_ready_from_begin(mode: Mode) -> SampleCount {
        // TODO set this to a reasonable value once FCFB delay and SDR delay
        // are properly compensated for and timing relationship between TX and RX
        // is actually correct.
        //const UL_OFFSET_SYMBOLS: SampleCount = 16;
        const UL_OFFSET_SYMBOLS: SampleCount = 0;
        match mode {
            Mode::Idle | // ?
            Mode::Dl | Mode::DlUnsynchronized =>
                // Begin the window half a sample before slot beginning
                // to leave room for timing errors.
                SAMPLES_SLOT - SAMPLES_SYMBOL / 2,
            Mode::Ul =>
                // Wait a bit past the end of the slot to leave more room for path delay.
                // This also prevents detecting training sequences earlier than expected.
                SAMPLES_SLOT + UL_OFFSET_SYMBOLS * SAMPLES_SYMBOL,
        }
    }

    /// Process one input sample
    pub fn sample(&mut self, input: ComplexSample, sample_counter: SampleCount) {
        // Lost samples during a slot would cause some symbols to be
        // in the wrong position in the history buffer.
        // For up to 1 slot of lost samples, feed zeros into the demodulator,
        // so it will only corrupt a part of the slot.
        // This also avoids skipping demodulation of a slot
        // due to a small number of lost samples.
        let samples_lost = sample_counter - self.next_input_sample_count;
        let samples_to_add = samples_lost.max(0).min(SAMPLES_SLOT);
        for i in -samples_to_add..0 {
            self.process_sample(ComplexSample::ZERO, sample_counter + i);
        }
        self.process_sample(input, sample_counter);
        self.next_input_sample_count = sample_counter + 1;
    }

    pub fn process_sample(&mut self, input: ComplexSample, sample_counter: SampleCount) {
        // Compensate for delay of matched filter in sample count
        let sample_counter = sample_counter - CHANNEL_FILTER_TAPS.len() as SampleCount;

        let filtered = self.matched_filter.sample(&CHANNEL_FILTER_TAPS, input);
        self.past_samples.write(filtered);
        self.past_samples_abs.write(filtered.abs());

        let tdiff = sample_counter - self.slot_ready_time;
        // Lost samples (gaps in sample_counter) or adjustment of reference_time
        // (particularly for uplink monitor getting reference_time from another demodulator)
        // might cause slot_ready_time to be in the past or far in the future.
        // Find the next future slot in these cases.
        if !(-2 * SAMPLES_SLOT..=0).contains(&tdiff) {
            // div_euclid always rounds towards negative numbers,
            // so use it with negations to round up to the next slot.
            let slots_to_skip = -(-tdiff).div_euclid(SAMPLES_SLOT) as i32;
            tracing::warn!("Skipping demodulation of {} slots due to lost samples", slots_to_skip);
            self.add_slots(slots_to_skip);
        }

        if sample_counter == self.slot_ready_time {
            self.process_past_samples();

            // Slot timing logic for different modes
            match self.mode {
                Mode::Idle => {}
                Mode::DlUnsynchronized => {
                    // When unsynchronized, try demodulating blocks more often
                    // so that they overlap.
                    // TODO: figure out maximum step that still makes sure
                    // a synchronization sequence gets detected in any position.
                    self.slot_ready_time += SAMPLES_SYMBOL * 100;
                }
                Mode::Dl => {
                    // Track slot timing.
                    // If timing seems to off by more than half a sample, adjust slot timing.
                    if self.averaged_timing_error > 0.5 {
                        self.reference_time += 1;
                        self.averaged_timing_error -= 1.0;
                    }
                    if self.averaged_timing_error < -0.5 {
                        self.reference_time -= 1;
                        self.averaged_timing_error += 1.0;
                    }

                    self.add_slots(1);
                }
                Mode::Ul => {
                    self.add_slots(1);
                }
            }
        }
    }

    fn process_past_samples(&mut self) {
        match self.mode {
            Mode::Idle => {}
            Mode::Ul => {
                // Look for a normal uplink burst
                self.process_slot(SPS * 256, 256, 0);
                // Look for control uplink bursts in both subslots
                self.process_slot(SPS * 256, 128, 1);
                self.process_slot(SPS * 256 - SPS * 255 / 2, 128, 2);
            }
            Mode::Dl | Mode::DlUnsynchronized => {
                self.process_slot(SPS * 256, 256, 0);
            } // TODO: a shorter window could be used for Mode::DlUnsynchronized
        }
    }

    /// Try to find bursts from a slot.
    /// First symbol is used as a phase reference for the first 2 demodulated bits,
    /// so number of bits demodulated is (n_symbols-1)*2.
    fn process_slot(&mut self, d: usize, n_symbols: usize, subslot_number: u8) {
        let symbol_timing = self.estimate_timing(d, SPS * n_symbols);
        let first_symbol_index = symbol_timing.floor() as usize;
        let timing_fract = symbol_timing.fract();

        let burst_finder = match subslot_number {
            0 => &mut self.full_slot,
            1 => &mut self.subslot1,
            2 => &mut self.subslot2,
            _ => unreachable!(),
        };

        burst_finder.clear();

        let bits = &mut burst_finder.bits;
        let mut previous_symbol: Option<ComplexSample> = None;
        for i in (first_symbol_index..first_symbol_index + SPS * n_symbols).step_by(SPS) {
            // Use fractional part of timing estimate to interpolate between samples.
            // Linear interpolation is not the best choice here but maybe good enough.
            let symbol = (1.0 - timing_fract) * self.past_samples.delayed(d - i) + timing_fract * self.past_samples.delayed(d - (i + 1));

            if let Some(previous_symbol) = previous_symbol {
                // Differential phase demodulation
                let diff = symbol * previous_symbol.conj();
                // Make decisions
                bits.push(if diff.im < 0.0 { 1 } else { 0 });
                bits.push(if diff.re < 0.0 { 1 } else { 0 });
            }
            previous_symbol = Some(symbol);
        }

        let mut training_sequence_found = false;
        //tracing::trace!("{} {:?}", symbol_timing, bits);

        match self.mode {
            Mode::DlUnsynchronized => {
                // Try to find a synchronization training sequence in bits.
                // Look for sequence only in positions after the nominal position,
                // so that we can switch to Mode::Dl and still demodulate the burst.
                let (pos, dist) = find_sequence(&bits[train_consts::SEQ_SYNC_OFFSET + 2..], &train_consts::SEQ_SYNC_AS_ARR);
                if dist <= SlotBurstFinder::SEQ_SYNC_MAX_ERRS {
                    tracing::info!(
                        "Found synchronization training sequence at {} with {} errors, switching to synchronized mode",
                        pos,
                        dist
                    );

                    // Set reference time such that this burst will get demodulated
                    // in slot number 0 after switching to Mode::Dl.
                    let slot_begin_time = self.slot_ready_time - Self::slot_ready_from_begin(Mode::Dl);
                    self.reference_time = slot_begin_time + (pos as SampleCount + 2 - 510) / 2 * SAMPLES_SYMBOL;

                    // Slot number gets incremented before the burst will be demodulated.
                    // Set initial value so that it becomes 0.
                    let first_slot = 0;
                    self.set_slot(TdmaTime::from_int(first_slot - 1));

                    // Uplink demodulator synchronizing to a downlink demodulator
                    // reads demodulated_slot_time to get an initial slot number.
                    // Set it here in case an uplink demodulator ends up reading it
                    // before the first downlink slot has been actually demodulated.
                    self.demodulated_slot_time = TdmaTime::from_int(first_slot);

                    // TODO: also consider symbol timing phase here
                    self.mode = Mode::Dl;
                    self.averaged_timing_error = 0.0;
                    self.slots_since_last_valid_burst = 0;
                    training_sequence_found = true;
                }
            }
            Mode::Dl => {
                training_sequence_found = burst_finder.check_slot(SlotType::Dl);

                // If no valid training sequences have been detected for some time,
                // assume signal is lost and switch back to Mode::DlUnsynchronized.
                if training_sequence_found {
                    self.slots_since_last_valid_burst = 0;
                } else {
                    self.slots_since_last_valid_burst += 1;
                    if self.slots_since_last_valid_burst >= 100 {
                        tracing::info!("Signal seems to be lost, starting to look for synchronization again");
                        self.mode = Mode::DlUnsynchronized;
                    }
                }

                self.demodulated_slot_time = self.current_slot;
                self.demodulated_slot_available = true;
            }
            Mode::Ul => {
                training_sequence_found = burst_finder.check_slot(if subslot_number == 0 { SlotType::UlFull } else { SlotType::UlSub });

                // Uplink slot numbering is offset from downlink by 2.
                // This could also be done by using a different reference_time for UL
                // but it seems simpler to do it here.
                self.demodulated_slot_time = self.current_slot.add_timeslots(-2);
                self.demodulated_slot_available = true;
            }
            Mode::Idle => unreachable!(),
        }

        if self.mode == Mode::Dl && training_sequence_found {
            // Try to keep symbol timing phase near SPS/2.
            // This leaves half a symbol margin for timing error
            // in both directions before we are off by one symbol.
            let timing_error = symbol_timing - (SPS as RealSample * 0.5);
            self.averaged_timing_error += (timing_error - self.averaged_timing_error) * 0.1;
        }
    }

    /// Estimate timing for a window of n samples
    /// starting from a sample d samples in the past.
    /// Return an index to the sample within the window
    /// from which the first symbol should be sampled.
    /// The value is a floating point number whose fractional part
    /// may be used to interpolate between samples.
    fn estimate_timing(&self, d: usize, n: usize) -> RealSample {
        // Taking the absolute value of samples
        // produces a tone at the symbol rate.
        // Use the phase of that tone to estimate symbol timing.
        //
        // To estimate the phase, multiply the signal by sine and cosine
        // waves, accumulate the results and take atan2.
        //
        // With 4 samples per symbol, these sine and cosine
        // waves are simply +1,0,-1,0 and so on, simplifying
        // the loop to only additions and subtractions.
        // If SPS is changed to something else, this code needs to be updated.
        assert!(SPS == 4);

        let mut sum_i: RealSample = 0.0;
        let mut sum_q: RealSample = 0.0;
        for i in (0..n).step_by(SPS) {
            sum_i += self.past_samples_abs.delayed(d - i) - self.past_samples_abs.delayed(d - (i + 2));
            sum_q += self.past_samples_abs.delayed(d - (i + 1)) - self.past_samples_abs.delayed(d - (i + 3));
        }
        // Scale result between 0 and SPS.
        let timing_phase = (sum_q.atan2(sum_i) * (SPS as RealSample / (2.0 * sample_consts::PI))).rem_euclid(SPS as RealSample);
        // rem_euclid may return rhs in some rare cases
        // (see https://doc.rust-lang.org/std/primitive.f32.html#method.rem_euclid )
        // so wrap it to 0 in that case.
        if timing_phase < SPS as RealSample { timing_phase } else { 0.0 }
    }

    /// Synchronize an uplink demodulator to a downlink demodulator
    /// for simultaneous UL/DL monitoring.
    pub fn sync_to_demodulator(&mut self, demod: &Demodulator) {
        if demod.mode == Mode::DlUnsynchronized {
            self.mode = Mode::Idle;
        }
        if demod.mode == Mode::Dl {
            self.reference_time = demod.reference_time;

            if self.mode == Mode::Idle {
                self.current_slot = demod.demodulated_slot_time;
            }

            self.mode = Mode::Ul;
        }
    }

    pub fn demodulated_slot_available(&self) -> bool {
        self.demodulated_slot_available
    }

    pub fn take_demodulated_slot<'a>(&'a mut self) -> Option<RxSlotBits<'a>> {
        if self.demodulated_slot_available {
            self.demodulated_slot_available = false;
            Some(RxSlotBits {
                time: self.demodulated_slot_time,
                slot: self.full_slot.get_burst(),
                subslot1: self.subslot1.get_burst(),
                subslot2: self.subslot2.get_burst(),
            })
        } else {
            None
        }
    }
}

fn hamming_distance(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).map(|(a_bit, b_bit)| if a_bit != b_bit { 1 } else { 0 }).sum()
}

/// Find the position in bits which looks most like the sequence.
/// Return the position and hamming distance.
/// Step in multiples of 2 bits because offset is always whole symbols.
fn find_sequence(bits: &[u8], sequence: &[u8]) -> (usize, usize) {
    let mut min_dist = sequence.len();
    let mut min_pos = 0;
    for (position, window) in bits.windows(sequence.len()).enumerate().step_by(2) {
        let dist = hamming_distance(window, sequence);
        if dist < min_dist {
            min_dist = dist;
            min_pos = position;
        }
    }
    (min_pos, min_dist)
}

enum SlotType {
    /// Downlink slot
    Dl,
    /// Uplink full slot
    UlFull,
    /// Uplink subslot
    UlSub,
}

struct SlotBurstFinder {
    /// Demodulated bits of a slot
    bits: Vec<u8>,
    /// Training sequence found
    train_type: TrainingSequence,
    /// Number of bit errors in training sequence
    train_errs: usize,
    /// Position in bits where a burst begins
    burst_pos: usize,
    /// Length of burst
    burst_len: usize,
}

impl SlotBurstFinder {
    const ERRS_NO_BURST: usize = 100;

    // const SEQ_NORM_DL_MAX_ERRS: usize = 2;
    // const SEQ_NORM_UL_MAX_ERRS: usize = 2;
    // const SEQ_EXT_MAX_ERRS: usize = 2;
    // const SEQ_SYNC_MAX_ERRS: usize = 3;

    const SEQ_NORM_DL_MAX_ERRS: usize = 1;
    const SEQ_NORM_UL_MAX_ERRS: usize = 1;
    const SEQ_EXT_MAX_ERRS: usize = 1;
    const SEQ_SYNC_MAX_ERRS: usize = 1;

    fn new() -> Self {
        Self {
            bits: Vec::with_capacity(510),
            train_type: TrainingSequence::NotFound,
            train_errs: Self::ERRS_NO_BURST,
            burst_pos: 0,
            burst_len: 0,
        }
    }

    fn clear(&mut self) {
        self.bits.clear();
        self.train_type = TrainingSequence::NotFound;
        self.train_errs = Self::ERRS_NO_BURST;
        self.burst_pos = 0;
        self.burst_len = 0;
    }

    fn check_sequence(
        &mut self,
        train_pos_in_burst: usize,
        burst_len: usize,
        train_type: TrainingSequence,
        train_bits: &[u8],
        max_bit_errors: usize,
    ) -> bool {
        assert!(self.bits.len() >= burst_len);
        let min_burst_pos = 0;
        let max_burst_pos = self.bits.len() - burst_len;
        let min_train_pos = min_burst_pos + train_pos_in_burst;
        let max_train_pos = max_burst_pos + train_pos_in_burst;

        let train_len = train_bits.len();
        let (pos, dist) = find_sequence(&self.bits[min_train_pos..max_train_pos + train_len], train_bits);
        if dist <= max_bit_errors && dist < self.train_errs {
            let train_pos = min_train_pos + pos;
            self.burst_pos = train_pos - train_pos_in_burst;
            self.train_errs = dist;
            self.burst_len = burst_len;
            self.train_type = train_type;
            tracing::info!("Found {:?} at {} with {} errors", train_type, train_pos, dist);
            true
        } else {
            false
        }
    }

    fn check_slot(&mut self, slot_type: SlotType) -> bool {
        self.train_errs = Self::ERRS_NO_BURST;
        match slot_type {
            SlotType::Dl => {
                if self.check_sequence(
                    train_consts::SEQ_SYNC_OFFSET,
                    510,
                    TrainingSequence::SyncTrainSeq,
                    &train_consts::SEQ_SYNC_AS_ARR[..],
                    Self::SEQ_SYNC_MAX_ERRS,
                ) {
                    return true;
                }
                if self.check_sequence(
                    train_consts::SEQ_NORM_DL_OFFSET,
                    510,
                    TrainingSequence::NormalTrainSeq1,
                    &train_consts::SEQ_NORM1_AS_ARR[..],
                    Self::SEQ_NORM_DL_MAX_ERRS,
                ) {
                    return true;
                };
                if self.check_sequence(
                    train_consts::SEQ_NORM_DL_OFFSET,
                    510,
                    TrainingSequence::NormalTrainSeq2,
                    &train_consts::SEQ_NORM2_AS_ARR[..],
                    Self::SEQ_NORM_DL_MAX_ERRS,
                ) {
                    return true;
                };
            }
            SlotType::UlFull => {
                if self.check_sequence(
                    4 + 216,
                    4 + 216 + 22 + 216 + 4,
                    TrainingSequence::NormalTrainSeq1,
                    &train_consts::SEQ_NORM1_AS_ARR[..],
                    Self::SEQ_NORM_UL_MAX_ERRS,
                ) {
                    return true;
                };
                if self.check_sequence(
                    4 + 216,
                    4 + 216 + 22 + 216 + 4,
                    TrainingSequence::NormalTrainSeq2,
                    &train_consts::SEQ_NORM2_AS_ARR[..],
                    Self::SEQ_NORM_UL_MAX_ERRS,
                ) {
                    return true;
                };
            }
            SlotType::UlSub => {
                if self.check_sequence(
                    4 + 84,
                    4 + 84 + 30 + 84 + 4,
                    TrainingSequence::ExtendedTrainSeq,
                    &train_consts::SEQ_EXT_AS_ARR[..],
                    Self::SEQ_EXT_MAX_ERRS,
                ) {
                    return true;
                };
            }
        }
        false
    }

    fn get_burst<'a>(&'a mut self) -> RxBurstBits<'a> {
        RxBurstBits {
            train_type: self.train_type,
            bits: &self.bits[self.burst_pos..self.burst_pos + self.burst_len],
        }
    }
}
