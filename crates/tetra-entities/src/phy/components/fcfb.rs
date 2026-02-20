use num::Zero;
use rustfft;
use std::sync::Arc;
use std::vec::Vec;

use super::dsp_types::*;

pub type BlockCount = i64;
type Weights = Arc<[RealSample]>;

// ------------------------------------------------
// Buffering helper for both analysis and synthesis
// ------------------------------------------------

#[derive(Copy, Clone)]
pub struct InputBlockSize {
    /// Number of new input samples in each input block.
    pub new: usize,
    /// Number of overlapping samples between consecutive blocks.
    /// The first "overlap" samples of a block
    /// should be the same as the last samples of the previous block.
    /// Total number of samples in each input block
    /// is equal to the sum of "new" and "overlap".
    pub overlap: usize,
}

pub struct InputBuffer {
    size: InputBlockSize,
    buffer: Vec<ComplexSample>,
}

impl InputBuffer {
    pub fn new(size: InputBlockSize) -> Self {
        Self {
            size,
            buffer: vec![ComplexSample::ZERO; size.new + size.overlap],
        }
    }

    /// Prepare buffer for a new input block.
    /// Return a slice for writing new input samples.
    pub fn prepare_for_new_samples(&mut self) -> &mut [ComplexSample] {
        // Move overlapping part from the end of the previous block to the beginning
        self.buffer.copy_within(self.size.new..self.size.new + self.size.overlap, 0);
        // Return slice for writing new samples
        self.buffer_in()
    }

    /// Return a slice which can be passed to the process() method of a filter bank.
    pub fn buffer(&self) -> &[ComplexSample] {
        &self.buffer[..]
    }

    /// Return a slice for writing new samples into the buffer.
    /// This is the same as the one returned by the latest prepare_for_new_samples call.
    pub fn buffer_in(&mut self) -> &mut [ComplexSample] {
        &mut self.buffer[self.size.overlap..self.size.new + self.size.overlap]
    }
}

// -------------------------------------------
// Common code for both analysis and synthesis
// -------------------------------------------

/// Overlap factor
#[derive(Copy, Clone, PartialEq)]
pub enum Overlap {
    // Overlap factor of 1/2
    O1_2,
    // Overlap factor of 1/4
    O1_4,
}

/// Overlapping amount needs to be an integer number of samples.
/// This means FFT size must be a multiple of the denominator
/// of overlap factor.
fn required_fft_size_factor(overlap: Overlap) -> usize {
    match overlap {
        Overlap::O1_2 => 2,
        Overlap::O1_4 => 4,
    }
}

/// Check that FFT size is a multiple of required_fft_size_factor.
/// For now it panics but the code could be changed to return errors too.
fn check_fft_size(fft_size: usize, overlap: Overlap) {
    let required = required_fft_size_factor(overlap);
    if fft_size % required != 0 {
        panic!("FFT size must be a multiple of {}. {} is not.", required, fft_size);
    }
}

/// Compute input block size for a given FFT/IFFT size and overlap factor.
fn input_block_size(fft_size: usize, overlap: Overlap) -> InputBlockSize {
    match overlap {
        Overlap::O1_2 => InputBlockSize {
            new: fft_size / 2,
            overlap: fft_size / 2,
        },
        Overlap::O1_4 => InputBlockSize {
            new: fft_size / 4 * 3,
            overlap: fft_size / 4,
        },
    }
}

fn slice_middle_samples(samples: &[ComplexSample], overlap: Overlap) -> &[ComplexSample] {
    let len = samples.len();
    let (first_sample, n_samples) = match overlap {
        Overlap::O1_2 => ((len + 2) / 4, len / 2),
        Overlap::O1_4 => ((len + 4) / 8, len / 4 * 3),
    };
    &samples[first_sample..first_sample + n_samples]
}

/// Compute phase rotation for a given center bin number, block counter and overlap factor.
/// All bins in a block will be phase shifted by the amount returned.
/// Return value is a number from 0 to 3, where:
/// 0 means 0° phase shift. Values are not affected.
/// 1 means 90° phase shift. Values are multipled by i.
/// 2 means 180° phase shift. Values are multipled by -1.
/// 3 means 270° phase shift. Values are multipled by -i.
fn get_phase_rotation(center_bin: isize, block_count: BlockCount, overlap: Overlap) -> i8 {
    (center_bin.rem_euclid(4) as i8
        * block_count.rem_euclid(4) as i8
        * match overlap {
            Overlap::O1_2 => 2,
            Overlap::O1_4 => 1,
        })
    .rem_euclid(4)
}

// ----------------------------------------
//           Analysis filter bank
// ----------------------------------------

#[derive(Copy, Clone)]
pub struct AnalysisInputParameters {
    pub fft_size: usize,
    /// Input sample rate.
    pub sample_rate: f64,
    /// Input center frequency.
    pub center_frequency: f64,
    /// Overlap factor
    pub overlap: Overlap,
}

pub struct AnalysisIntermediateResult {
    fft_result: Vec<ComplexSample>,
    /// Block counter to implement output phase rotation.
    count: BlockCount,
}

/// Fast-convolution analysis filter bank.
pub struct AnalysisInputProcessor {
    parameters: AnalysisInputParameters,
    fft_plan: Arc<dyn rustfft::Fft<RealSample>>,
    result: AnalysisIntermediateResult,
}

impl AnalysisInputProcessor {
    pub fn new(fft_planner: &mut rustfft::FftPlanner<RealSample>, parameters: AnalysisInputParameters) -> Self {
        check_fft_size(parameters.fft_size, parameters.overlap);
        Self {
            parameters,
            fft_plan: fft_planner.plan_fft_forward(parameters.fft_size),
            result: AnalysisIntermediateResult {
                fft_result: vec![ComplexSample::ZERO; parameters.fft_size],
                count: 0,
            },
        }
    }

    pub fn input_block_size(&self) -> InputBlockSize {
        input_block_size(self.parameters.fft_size, self.parameters.overlap)
    }

    pub fn make_input_buffer(&self) -> InputBuffer {
        InputBuffer::new(self.input_block_size())
    }

    /// Input samples should overlap between consequent blocks.
    /// The first "overlap" samples of a block
    /// should be the same as the last samples of the previous block.
    /// The numbers of samples are returned by the input_block_size()
    /// method, returning an AnalysisInputBlockSize struct.
    ///
    /// Caller may implement overlap in any way it wants, for example,
    /// by giving overlapping slices from a larger buffer,
    /// or by copying the end of the previous block
    /// to the beginning of the current block.
    /// The latter can be done using the AnalysisInputBuffer struct
    /// which can be constructed using the make_input_buffer() method.
    ///
    /// block_count should increment by 1 for each processing block.
    /// It is used to implement blockwise phase rotation
    /// (see https://ieeexplore.ieee.org/document/6834830).
    /// Passing it as a parameter allows input blocks to be skipped
    /// (for example, due to missing samples from a receiver)
    /// while keeping correct phase relationship between blocks.
    pub fn process(&mut self, input: &[ComplexSample], block_count: BlockCount) -> &AnalysisIntermediateResult {
        self.result.fft_result.copy_from_slice(input);
        self.fft_plan.process(&mut self.result.fft_result[..]);
        self.result.count = block_count;

        &self.result
    }
}

#[derive(Clone)]
pub struct AnalysisOutputParameters {
    pub center_bin: isize,
    pub weights: Weights,
}

impl AnalysisOutputParameters {
    /// Design analysis bank output parameters
    /// for a given output sample rate and frequency.
    /// If bandwidth is given, it will determine the width
    /// of the flat part of the frequency response.
    pub fn for_frequency(
        analysis_in_params: AnalysisInputParameters,
        output_sample_rate: f64,
        output_center_frequency: f64,
        bandwidth: Option<f64>,
    ) -> Self {
        let ifft_size = (output_sample_rate * analysis_in_params.fft_size as f64 / analysis_in_params.sample_rate).round() as usize;

        let center_bin = (((output_center_frequency - analysis_in_params.center_frequency) * analysis_in_params.fft_size as f64
            / analysis_in_params.sample_rate)
            .round() as isize)
            .rem_euclid(analysis_in_params.fft_size as isize);

        Self {
            center_bin,
            weights: raised_cosine_weights_default(
                ifft_size,
                bandwidth
                    .map(|bandwidth| (bandwidth * analysis_in_params.fft_size as f64 / analysis_in_params.sample_rate).round() as usize),
                None,
                analysis_in_params.overlap,
            ),
        }
    }
}

pub struct AnalysisOutputProcessor {
    input_parameters: AnalysisInputParameters,
    parameters: AnalysisOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<RealSample>>,
    buffer: Vec<ComplexSample>,
    /// Scaling factor to get unity gain in passband.
    scaling: RealSample,
}

impl AnalysisOutputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        input_parameters: AnalysisInputParameters,
        parameters: AnalysisOutputParameters,
    ) -> Self {
        let ifft_size = parameters.weights.len();
        check_fft_size(ifft_size, input_parameters.overlap);
        Self {
            input_parameters,
            parameters: parameters.clone(),
            ifft_plan: fft_planner.plan_fft_inverse(ifft_size),
            buffer: vec![ComplexSample::ZERO; ifft_size],
            scaling: 1.0 / input_parameters.fft_size as RealSample,
        }
    }

    pub fn process(&mut self, intermediate_result: &AnalysisIntermediateResult) -> &[ComplexSample] {
        assert!(intermediate_result.fft_result.len() == self.input_parameters.fft_size);

        let phasenum = get_phase_rotation(self.parameters.center_bin, intermediate_result.count, self.input_parameters.overlap);

        // Convert to scaling factor and multiply_by_i value.
        let scaling = if phasenum >= 2 { -self.scaling } else { self.scaling };
        let multiply_by_i = phasenum % 2 == 1;

        // Or should we just make scaling factor a complex number?
        /*let scaling = self.scaling * match phasenum {
            0 => ComplexSample { re:  1.0, im:  0.0 },
            1 => ComplexSample { re:  0.0, im:  1.0 },
            2 => ComplexSample { re: -1.0, im:  0.0 },
            3 => ComplexSample { re:  0.0, im: -1.0 },
            _ => panic!("Bug"),
        };*/

        let fft_size = self.input_parameters.fft_size;
        let ifft_size = self.buffer.len();
        let half_size = (ifft_size / 2) as isize;

        // This could probably be optimized a lot.
        // Now it computes each index using modulos which might be slow.
        for bin_number in -half_size..half_size {
            let bin_index_in = (self.parameters.center_bin + bin_number).rem_euclid(fft_size as isize) as usize;
            let bin_index_out = bin_number.rem_euclid(ifft_size as isize) as usize;
            // Apply weight
            let weighted = self.parameters.weights[bin_index_out] * intermediate_result.fft_result[bin_index_in] * scaling;
            // Apply 90° phase rotation if needed.
            self.buffer[bin_index_out] = if multiply_by_i {
                ComplexSample {
                    re: -weighted.im,
                    im: weighted.re,
                }
            } else {
                weighted
            }

            // Or should we just make scaling factor a complex number?
            //self.buffer[bin_index_out] = self.parameters.weights[bin_index_out] * intermediate_result.fft_result[bin_index_in] * scaling;
        }

        self.ifft_plan.process(&mut self.buffer);

        slice_middle_samples(&self.buffer, self.input_parameters.overlap)
    }

    pub fn new_with_frequency(
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        analysis_in_params: AnalysisInputParameters,
        output_sample_rate: f64,
        output_center_frequency: f64,
        bandwidth: Option<f64>,
    ) -> Self {
        AnalysisOutputProcessor::new(
            fft_planner,
            analysis_in_params,
            AnalysisOutputParameters::for_frequency(analysis_in_params, output_sample_rate, output_center_frequency, bandwidth),
        )
    }
}

// ----------------------------------------
//          Synthesis filter bank
// ----------------------------------------

#[derive(Copy, Clone)]
pub struct SynthesisOutputParameters {
    pub ifft_size: usize,
    /// Output sample rate of synthesis bank.
    pub sample_rate: f64,
    /// Output center frequency of synthesis bank.
    pub center_frequency: f64,
    /// Overlap factor
    pub overlap: Overlap,
}

pub struct SynthesisOutputProcessor {
    parameters: SynthesisOutputParameters,
    ifft_plan: Arc<dyn rustfft::Fft<RealSample>>,
    /// Buffer for FFT processing.
    /// The buffer is used to accumulate filter bank inputs
    /// (in frequency domain) before IFFT, and
    /// to store output signal (in time domain) after IFFT.
    /// buffer_state indicates what the buffer currently contains.
    buffer: Vec<ComplexSample>,
    buffer_state: SynthesisBufferState,
}

#[derive(PartialEq)]
enum SynthesisBufferState {
    /// Buffer is full of zeros.
    /// This is the case if no inputs have been added
    /// since the last call to process.
    Clear,
    /// Buffer contains inputs to IFFT in frequency domain,
    /// that is, one or more inputs have been added.
    Input,
    /// Buffer contains IFFT output.
    /// A slice of the buffer is used to return output signal.
    Output,
}

pub struct SynthesisIntermediateResult {
    /// Where in synthesis output IFFT input buffer
    /// to add the input FFT results.
    /// Output IFFT bin indexes are input FFT bin index + offset.
    offset: usize,
    fft_result: Vec<ComplexSample>,
}

impl SynthesisOutputProcessor {
    pub fn new(fft_planner: &mut rustfft::FftPlanner<RealSample>, parameters: SynthesisOutputParameters) -> Self {
        check_fft_size(parameters.ifft_size, parameters.overlap);
        Self {
            parameters,
            ifft_plan: fft_planner.plan_fft_inverse(parameters.ifft_size),
            buffer: vec![ComplexSample::ZERO; parameters.ifft_size],
            buffer_state: SynthesisBufferState::Clear,
        }
    }

    pub fn clear(&mut self) {
        for b in self.buffer.iter_mut() {
            *b = ComplexSample::ZERO;
        }
        self.buffer_state = SynthesisBufferState::Clear;
    }

    pub fn add(&mut self, intermediate_result: &SynthesisIntermediateResult) {
        // If previous result is still in the buffer, clear it
        // before starting to add inputs.
        // This happens for the first input added to a block.
        if self.buffer_state == SynthesisBufferState::Output {
            self.clear();
        }

        let ifft_size = self.buffer.len();
        for (index, value) in intermediate_result.fft_result.iter().enumerate() {
            // TODO: handle wrap-around without computing a modulo for each bin
            let out_index = (intermediate_result.offset + index).rem_euclid(ifft_size);
            self.buffer[out_index] += value;
        }

        self.buffer_state = SynthesisBufferState::Input;
    }

    pub fn process(&mut self) -> &[ComplexSample] {
        match self.buffer_state {
            SynthesisBufferState::Clear => {
                // No inputs have been added. Buffer is full of zeros.
                // IFFT of zeros is still zeros, so we can skip processing
                // and just return those zeros as the result.
            }
            SynthesisBufferState::Input => {
                // The usual case: buffer contains some inputs and
                // now it is time to process them to get the result.
                self.ifft_plan.process(&mut self.buffer);
                self.buffer_state = SynthesisBufferState::Output;
            }
            SynthesisBufferState::Output => {
                // No inputs have been added since the last call to process.
                // The buffer still contains the previous result though,
                // so clear it and return those zeros.
                self.clear();
            }
        }

        slice_middle_samples(&self.buffer, self.parameters.overlap)
    }

    pub fn output_block_size(&self) -> usize {
        slice_middle_samples(&self.buffer, self.parameters.overlap).len()
    }
}

#[derive(Clone)]
pub struct SynthesisInputParameters {
    pub center_bin: isize,
    pub weights: Weights,
}

impl SynthesisInputParameters {
    /// Design synthesis bank input parameters
    /// for a given input sample rate and frequency.
    pub fn for_frequency(
        output_parameters: SynthesisOutputParameters,
        input_sample_rate: f64,
        input_center_frequency: f64,
        bandwidth: Option<f64>,
    ) -> Self {
        let fft_size = (input_sample_rate * output_parameters.ifft_size as f64 / output_parameters.sample_rate).round() as usize;

        let center_bin = (((input_center_frequency - output_parameters.center_frequency) * output_parameters.ifft_size as f64
            / output_parameters.sample_rate)
            .round() as isize)
            .rem_euclid(output_parameters.ifft_size as isize);

        Self {
            center_bin,
            weights: raised_cosine_weights_default(
                fft_size,
                bandwidth
                    .map(|bandwidth| (bandwidth * output_parameters.ifft_size as f64 / output_parameters.sample_rate).round() as usize),
                None,
                output_parameters.overlap,
            ),
        }
    }
}

pub struct SynthesisInputProcessor {
    weights: Weights,
    fft_plan: Arc<dyn rustfft::Fft<RealSample>>,
    result: SynthesisIntermediateResult,
    /// This is a bit redundant since result.offset contains the information
    /// already, but it simplifies phase code rotation for now, maybe...
    center_bin: isize,
    /// Scaling factor for unity gain in passband.
    /// This could be included in weights to avoid some
    /// multiplications but that might complicate other things.
    /// Have to think about it a bit more.
    scaling: RealSample,
    overlap: Overlap,
}

impl SynthesisInputProcessor {
    pub fn new(
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        output_parameters: SynthesisOutputParameters,
        parameters: SynthesisInputParameters,
    ) -> Self {
        let fft_size = parameters.weights.len();
        check_fft_size(fft_size, output_parameters.overlap);
        Self {
            weights: parameters.weights,
            fft_plan: fft_planner.plan_fft_forward(fft_size),
            result: SynthesisIntermediateResult {
                offset: (parameters.center_bin - (fft_size / 2) as isize).rem_euclid(output_parameters.ifft_size as isize) as usize,
                fft_result: vec![ComplexSample::ZERO; fft_size],
            },
            center_bin: parameters.center_bin,
            scaling: 1.0 / (fft_size as RealSample),
            overlap: output_parameters.overlap,
        }
    }

    pub fn process(&mut self, input: &[ComplexSample], block_count: BlockCount) -> &SynthesisIntermediateResult {
        self.result.fft_result.copy_from_slice(input);
        self.fft_plan.process(&mut self.result.fft_result[..]);

        let phasenum = get_phase_rotation(self.center_bin, block_count, self.overlap);

        // Convert to scaling factor and multiply_by_i value.
        let scaling = if phasenum >= 2 { -self.scaling } else { self.scaling };
        let multiply_by_i = phasenum % 2 == 1;

        // Apply weights
        for (value, weight) in self.result.fft_result.iter_mut().zip(self.weights.iter()) {
            *value = *value * weight * scaling;
            // Apply 90° phase rotation if needed.
            if multiply_by_i {
                *value = ComplexSample {
                    re: value.im,
                    im: -value.re,
                };
            }
        }

        // Swap halves for simpler indexing when results are added
        // to IFFT input. This might not be the most efficient way to do it.
        let fft_size_half = self.result.fft_result.len() / 2;
        for i in 0..fft_size_half {
            self.result.fft_result.swap(i, fft_size_half + i);
        }

        &self.result
    }

    pub fn input_block_size(&self) -> InputBlockSize {
        input_block_size(self.result.fft_result.len(), self.overlap)
    }

    pub fn make_input_buffer(&self) -> InputBuffer {
        InputBuffer::new(self.input_block_size())
    }

    pub fn new_with_frequency(
        fft_planner: &mut rustfft::FftPlanner<RealSample>,
        output_parameters: SynthesisOutputParameters,
        input_sample_rate: f64,
        input_center_frequency: f64,
        bandwidth: Option<f64>,
    ) -> Self {
        Self::new(
            fft_planner,
            output_parameters,
            SynthesisInputParameters::for_frequency(output_parameters, input_sample_rate, input_center_frequency, bandwidth),
        )
    }
}

// ----------------------------------------
//          Filter bank design
// ----------------------------------------

/// Design raised cosine weights for a given IFFT size,
/// passband width and transition band width (given as number of bins).
pub fn raised_cosine_weights(ifft_size: usize, passband_bins: usize, transition_bins: usize) -> Weights {
    // I am not sure if it this would work correctly for an odd size,
    // but currently supported overlap factors needs an even IFFT size anyway.
    // Maybe returning an error instead of panicing with invalid values
    // would be better though.
    assert!(ifft_size % 2 == 0);

    let passband_half = passband_bins / 2 + 1;

    assert!(passband_half + transition_bins <= ifft_size / 2);

    let mut weights = vec![RealSample::zero(); ifft_size];
    for i in 0..passband_half {
        weights[i] = 1.0;
        if i != 0 {
            weights[ifft_size - i] = 1.0;
        }
    }
    for i in 0..transition_bins {
        let v = 0.5 + 0.5 * (sample_consts::PI * (i + 1) as RealSample / (transition_bins + 1) as RealSample).cos();
        let j = passband_half + i;
        weights[j] = v;
        if j != 0 {
            weights[ifft_size - j] = v;
        }
    }

    Weights::from(weights)
}

/// Design raised cosine weights for a given IFFT size,
/// passband width and transition band width (given as number of bins).
/// Use None for default values.
///
/// If passband_bins is Some and transition_bins is None (i.e. default),
/// transition band will be made as wide as possible.
/// This minimizes spurious products.
///
/// If passband_bins is None (i.e. default) and transition_bins is Some,
/// passband will be made as wide as possible.
///
/// If both are None, transition band width will get a default value
/// depending on overlap factor,
/// chosen to keep spurious products (TBD, at least 60?) dB down.
/// Passband will be made as wide as possible.
/// If IFFT size is too small to fit a transition band of the default width,
/// the whole bandwidth will be made transition band and there will be no
/// flat part in the frequency response.
pub fn raised_cosine_weights_default(
    ifft_size: usize,
    passband_bins: Option<usize>,
    transition_bins: Option<usize>,
    overlap: Overlap,
) -> Weights {
    let (p, t) = match (passband_bins, transition_bins) {
        (Some(p), Some(t)) => (p, t),
        (Some(p), None) => (p, (ifft_size - p / 2 * 2) / 2 - 1),
        (None, t) => {
            let t = t
                .unwrap_or(match overlap {
                    Overlap::O1_2 => 15,
                    // Smaller overlap factor needs a wider transition band
                    // for similar level of spurious products.
                    Overlap::O1_4 => 31,
                })
                .min(ifft_size / 2 - 1);
            (ifft_size - 2 - 2 * t + 1, t)
        }
    };

    raised_cosine_weights(ifft_size, p, t)
}
