/// Type used to represent input bits.
/// "0" is represented as -1, "1" as +1, and punctured bit as 0.
/// Soft decision decoding is also possible by using
/// higher negative values to represent more likely "0"
/// and higher positive values to represent more likely "1".
/// Too high values might cause path metrics to overflow though.
pub type SoftBit = i8;

/// Type used to accumulate path metrics.
/// 16 bits should be enough for our message lengths without need for renormalizations.
type Metric = i16;

/// Constraint length of the code.
/// This is defined as a constant rather than a const generic parameter
/// since it also affect NUM_STATES and the choice of type for DecisionBitmap
/// which would be more complicated to make generic.
/// If decoders for different constraint lengths are needed, it might be
/// easier to generate them using a macro instead.
const K: usize = 5;

const NUM_STATES: usize = num_states(K);

/// Unsigned integer type used to store decisions for each state in trellis.
/// Each bit represents a decision for a given state,
/// so the number of bits should be at least the number of states.
type DecisionBitmap = u16;

/// Number of states for a given constraint length.
pub const fn num_states(k: usize) -> usize {
    1 << (k - 1)
}

/// Viterbi decoder for a binary convolutional code of rate 1/N.
pub struct ViterbiDecoder<const N: usize> {
    /// Expected encoder outputs for each state for encoder input "0".
    expected_0: [[SoftBit; NUM_STATES]; N],
}

impl<const N: usize> ViterbiDecoder<N> {
    pub fn new_with_polynomials(generator_polynomials: &[[bool; K]; N]) -> Self {
        // Generate look-up table for expected encoder output bits.
        // With the generator polynomials used here, the expected outputs
        // for "1" input are inverse of those for "0" input,
        // so only generate the table for "0" and invert results later.
        let expected_0 = std::array::from_fn(|poly_n| {
            let poly = generator_polynomials[poly_n];
            std::array::from_fn(|state| {
                let mut encoder_output: bool = false;
                // Each bit of the state number corresponds to
                // a past input of the encoder.
                for bit_i in 0..K - 1 {
                    let past_input_bit = (state & (1 << (K - 2 - bit_i))) != 0;
                    if past_input_bit && poly[bit_i] {
                        encoder_output = !encoder_output;
                    }
                }
                if encoder_output { 1 as SoftBit } else { -1 as SoftBit }
            })
        });
        Self { expected_0 }
    }

    pub fn decode(&self, received_bits: &[SoftBit]) -> Vec<u8> {
        let num_output_bits = received_bits.len() / N;
        let mut trellis_decisions: Vec<DecisionBitmap> = Vec::with_capacity(num_output_bits);

        // Accumulated path metrics for each state.
        //
        // Encoder starts from state 0. Give that an initial metric of 0.
        // Give a very high initial value for other metrics
        // so they will not be chosen.
        // Use half of maximum possible value so there is still room
        // to accumulate on top of it without overflow.
        let mut metrics: [Metric; NUM_STATES] = [Metric::MAX / 2; NUM_STATES];
        metrics[0] = 0;

        for received_bits_for_one_output_bit in received_bits.chunks_exact(N) {
            // Branch metrics for encoder input "0".
            let mut branch_metrics_0: [Metric; NUM_STATES] = [0; NUM_STATES];

            // Loop through each generator polynomial and add to branch metrics
            for (received_bit, expected_0) in received_bits_for_one_output_bit.iter().zip(self.expected_0.iter()) {
                // Loop through each state
                for (branch_metric_0, expected_bit_0) in branch_metrics_0.iter_mut().zip(expected_0.iter()) {
                    *branch_metric_0 -= (received_bit * expected_bit_0) as Metric;
                }
            }

            let mut decisions: DecisionBitmap = 0;

            // New path metrics.
            metrics = std::array::from_fn(|state| {
                // Predecessor state if encoder input was 0.
                let predecessor_0 = (state * 2) % NUM_STATES;
                // Predecessor state if encoder input was 1.
                let predecessor_1 = predecessor_0 + 1;
                // Candidates for new path metrics.
                let metric_0 = metrics[predecessor_0] + branch_metrics_0[state];
                // With the generator polynomials used here, the expected
                // encoder outputs for encoder input "1" are inverse
                // of those for "0", so we can get branch metrics
                // for "1" input by using inverted metrics of "0" input.
                let metric_1 = metrics[predecessor_1] - branch_metrics_0[state];

                if metric_1 < metric_0 {
                    // We only need to store the decision as a single bit
                    // rather than the whole predecessor state number,
                    // since each state only has two possible predecessors.
                    // This is then converted back to state numbers in traceback.
                    decisions |= 1 << state;
                    metric_1
                } else {
                    metric_0
                }
            });
            trellis_decisions.push(decisions);
        }

        // Traceback

        // Tail bits should ensure the final state of the encoder is 0.
        let mut best_state = 0;

        let mut decoded_bits: Vec<u8> = Vec::with_capacity(num_output_bits);
        for decisions in trellis_decisions.iter().rev() {
            decoded_bits.push(((best_state >> (K - 2)) & 1) as u8);
            best_state = best_state * 2 % NUM_STATES + ((*decisions >> best_state) & 1) as usize;
        }
        decoded_bits.reverse();
        decoded_bits
    }
}

/// Decoder for rate 1/4 mother code
pub type TetraViterbiDecoder = ViterbiDecoder<4>;

impl TetraViterbiDecoder {
    pub fn new() -> Self {
        Self::new_with_polynomials(&[
            [true, true, false, false, true],
            [true, false, true, true, true],
            [true, true, true, false, true],
            [true, true, false, true, true],
        ])
    }
}

/// Decoder for rate 1/3 mother code used by TETRA speech codec
pub type TetraCodecViterbiDecoder = ViterbiDecoder<3>;

impl TetraCodecViterbiDecoder {
    pub fn new() -> Self {
        Self::new_with_polynomials(&[
            [true, true, true, true, true],
            [true, true, false, true, true],
            [true, false, true, false, true],
        ])
    }
}

/// Convenience wrapper for decoding SB1 blocks.
pub fn dec_sb1(in_buf: &[u8], out_buf: &mut [u8], sym_count: usize) {
    const MAX_SYM: usize = 864;
    assert!(sym_count <= MAX_SYM, "sym_count too large");
    assert!(in_buf.len() >= sym_count * 4, "in_buf too short");
    assert!(out_buf.len() >= sym_count, "out_buf too short");

    let soft: Vec<SoftBit> = in_buf[..sym_count * 4]
        .iter()
        .map(|&b| match b {
            0x00 => -1, // strong '0'
            0x01 => 1,  // strong '1'
            0xff => 0,  // erasure / puncture
            _ => panic!("viterbi_dec_sb1_wrapper: invalid input"),
        })
        .collect();

    let decoder = TetraViterbiDecoder::new();
    let decoded = decoder.decode(&soft);
    out_buf[..sym_count].copy_from_slice(&decoded[..sym_count]);
}

#[cfg(test)]
mod tests {
    use super::super::convenc;
    use super::*;
    use rand;

    #[test]
    fn test_decoder() {
        // Generate a random message with 4 zero tail bits
        let message: Vec<u8> = (0..288).map(|_| rand::random_range(0..2)).chain((0..4).map(|_| 0)).collect();
        eprintln!("Message: {:?}", message);

        let mut encoder = convenc::ConvEncState::new();
        let mut encoded = vec![0u8; message.len() * 4];
        encoder.encode(&message[..], &mut encoded[..]);

        // Convert to the format used by the decoder.
        // Puncture some bits, not really following any TETRA puncturing pattern,
        // but enough to check that the decoder can correct for missing bits.
        let encoded_soft: Vec<i8> = encoded
            .into_iter()
            .enumerate()
            .map(|(i, bit)| {
                if i % 3 > 0 {
                    0 // puncture
                } else if bit != 0 {
                    1
                } else {
                    -1
                }
            })
            .collect();

        let decoder = TetraViterbiDecoder::new();
        let decoded_message = decoder.decode(&encoded_soft[..]);

        eprintln!("Decoded message: {:?}", decoded_message);
        assert!(decoded_message[..] == message[..]);
    }
}
