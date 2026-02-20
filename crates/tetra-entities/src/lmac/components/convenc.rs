// Convolutional encoder and puncturing for TETRA

/// Puncturing rates
#[repr(usize)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RcpcPunctMode {
    Rate2_3 = 0,
    Rate1_3 = 1,
    Rate292_432 = 2,
    Rate148_432 = 3,
    Rate112_168 = 4,
    Rate72_162 = 5,
    Rate38_80 = 6,
}

/// State for the rate-1/2 “mother code” convolutional encoder.
#[derive(Clone, Copy, Debug)]
pub struct ConvEncState {
    delayed: [u8; 4],
}

impl ConvEncState {
    /// Create a new encoder state (all zeros).
    #[inline]
    pub fn new() -> Self {
        Self { delayed: [0; 4] }
    }

    /// Reset to all-zero state.
    #[inline]
    pub fn reset(&mut self) {
        self.delayed = [0; 4];
    }

    /// Encode a single input bit into four output bits.
    /// Writes into `out[0..4]`, returns the packed nibble `g1 | g2<<1 | g3<<2 | g4<<3`.
    #[inline(always)]
    fn encode_bit(&mut self, bit: u8, out: &mut [u8; 4]) -> u8 {
        let d0 = self.delayed[0];
        let d1 = self.delayed[1];
        let d2 = self.delayed[2];
        let d3 = self.delayed[3];

        // taps are XORs of bit and delayed state
        let g1 = bit ^ d0 ^ d3;
        let g2 = bit ^ d1 ^ d2 ^ d3;
        let g3 = bit ^ d0 ^ d1 ^ d3;
        let g4 = bit ^ d0 ^ d2 ^ d3;

        // shift register
        self.delayed[3] = d2;
        self.delayed[2] = d1;
        self.delayed[1] = d0;
        self.delayed[0] = bit;

        out[0] = g1;
        out[1] = g2;
        out[2] = g3;
        out[3] = g4;

        g1 | (g2 << 1) | (g3 << 2) | (g4 << 3)
    }

    /// Encode a sequence of bits (`input.len()` bytes, one bit each) into
    /// `4 * input.len()` output bits in `output` (rate 1/4 mother code).
    /// Panics if `output.len() < input.len() * 4`.
    pub fn encode(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(output.len() >= input.len() * 4);
        for (i, &bit) in input.iter().enumerate() {
            // safely coerce the 4‐byte window into `[u8;4]`
            let out_chunk: &mut [u8; 4] = (&mut output[i * 4..i * 4 + 4]).try_into().unwrap();
            self.encode_bit(bit, out_chunk);
        }
    }

    /// Encode a sequence of bits using rate 1/3 mother code (EN 300 395-2 Section 5.4.3.1).
    /// Uses generators G1, G2, G3 only (drops G4).
    /// Produces `3 * input.len()` output bits in `output`.
    /// Panics if `output.len() < input.len() * 3`.
    pub fn encode_rate1_3(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(output.len() >= input.len() * 3);
        let mut g4 = [0u8; 4];
        for (i, &bit) in input.iter().enumerate() {
            self.encode_bit(bit, &mut g4);
            // Rate 1/3: keep G1, G2, G3 only (indices 0, 1, 2)
            output[i * 3] = g4[0]; // G1
            output[i * 3 + 1] = g4[1]; // G2
            output[i * 3 + 2] = g4[2]; // G3
        }
    }
}

/// Speech convolutional encoder (EN 300 395-2, §5.4.3.1).
///
/// Uses DIFFERENT generator polynomials from CCH (EN 300 392-2, §8.2.3.1.1):
///   Speech: G1(D) = 1 + D + D² + D³ + D⁴,  G2(D) = 1 + D + D³ + D⁴,  G3(D) = 1 + D² + D⁴
///   CCH:    G1(D) = 1 + D + D⁴,  G2(D) = 1 + D² + D³ + D⁴,  G3(D) = 1 + D + D² + D⁴,  G4(D) = 1 + D + D³ + D⁴
#[derive(Clone, Copy, Debug)]
pub struct SpeechConvEncState {
    delayed: [u8; 4],
}

impl SpeechConvEncState {
    pub fn new() -> Self {
        Self { delayed: [0; 4] }
    }

    /// Encode a single input bit into three output bits (rate 1/3).
    #[inline(always)]
    fn encode_bit(&mut self, bit: u8, out: &mut [u8; 3]) {
        let d0 = self.delayed[0];
        let d1 = self.delayed[1];
        let d2 = self.delayed[2];
        let d3 = self.delayed[3];

        // G1 = 1 + D + D² + D³ + D⁴
        let g1 = bit ^ d0 ^ d1 ^ d2 ^ d3;
        // G2 = 1 + D + D³ + D⁴
        let g2 = bit ^ d0 ^ d2 ^ d3;
        // G3 = 1 + D² + D⁴
        let g3 = bit ^ d1 ^ d3;

        // shift register
        self.delayed[3] = d2;
        self.delayed[2] = d1;
        self.delayed[1] = d0;
        self.delayed[0] = bit;

        out[0] = g1;
        out[1] = g2;
        out[2] = g3;
    }

    /// Encode a sequence of bits with the speech rate 1/3 mother code.
    /// Produces `3 * input.len()` output bits.
    pub fn encode(&mut self, input: &[u8], output: &mut [u8]) {
        assert!(output.len() >= input.len() * 3);
        let mut g3 = [0u8; 3];
        for (i, &bit) in input.iter().enumerate() {
            self.encode_bit(bit, &mut g3);
            output[i * 3] = g3[0];
            output[i * 3 + 1] = g3[1];
            output[i * 3 + 2] = g3[2];
        }
    }
}

type IFunc = fn(u32) -> u32;

#[inline(always)]
const fn i_equals(j: u32) -> u32 {
    j
}

#[inline(always)]
const fn i_292(j: u32) -> u32 {
    j + ((j - 1) / 65)
}

#[inline(always)]
const fn i_148(j: u32) -> u32 {
    j + ((j - 1) / 35)
}

/// Puncturer parameters
#[derive(Copy, Clone)]
struct Puncturer {
    /// Puncturing pattern indices
    p: &'static [u32],
    /// puncturing period t
    t: u32,
    /// interleaving period
    period: u32,
    /// index mapping function
    i_func: IFunc,
}

// P-arrays
const P_RATE2_3: &[u32] = &[0, 1, 2, 5];
const P_RATE1_3: &[u32] = &[0, 1, 2, 3, 5, 6, 7];
const P_RATE8_12: &[u32] = &[0, 1, 2, 4];
const P_RATE8_18: &[u32] = &[0, 1, 2, 3, 4, 5, 7, 8, 10, 11];
const P_RATE8_17: &[u32] = &[0, 1, 2, 3, 4, 5, 7, 8, 10, 11, 13, 14, 16, 17, 19, 20, 22, 23];

// Get puncturer parameters by enum type
fn get_puncturer(pu: RcpcPunctMode) -> Puncturer {
    const PUNCTURERS: [Puncturer; 7] = [
        Puncturer {
            p: P_RATE2_3,
            t: 3,
            period: 8,
            i_func: i_equals,
        },
        Puncturer {
            p: P_RATE1_3,
            t: 6,
            period: 8,
            i_func: i_equals,
        },
        Puncturer {
            p: P_RATE2_3,
            t: 3,
            period: 8,
            i_func: i_292,
        },
        Puncturer {
            p: P_RATE1_3,
            t: 6,
            period: 8,
            i_func: i_148,
        },
        Puncturer {
            p: P_RATE8_12,
            t: 3,
            period: 6,
            i_func: i_equals,
        },
        Puncturer {
            p: P_RATE8_18,
            t: 9,
            period: 12,
            i_func: i_equals,
        },
        Puncturer {
            p: P_RATE8_17,
            t: 17,
            period: 24,
            i_func: i_equals,
        },
    ];

    match pu {
        RcpcPunctMode::Rate2_3 => PUNCTURERS[0],
        RcpcPunctMode::Rate1_3 => PUNCTURERS[1],
        RcpcPunctMode::Rate292_432 => PUNCTURERS[2],
        RcpcPunctMode::Rate148_432 => PUNCTURERS[3],
        RcpcPunctMode::Rate112_168 => PUNCTURERS[4],
        RcpcPunctMode::Rate72_162 => PUNCTURERS[5],
        RcpcPunctMode::Rate38_80 => PUNCTURERS[6],
    }
}

/// Puncture the `input` mother‐code bits into `output` of length `output.len()`.
pub fn get_punctured_rate(pu: RcpcPunctMode, input: &[u8], output: &mut [u8]) {
    let puncturer = get_puncturer(pu);
    let t = puncturer.t;
    let per = puncturer.period;
    let p = puncturer.p;
    let len = output.len() as u32;
    for j in 1..=len {
        let i = (puncturer.i_func)(j);
        let blk = (i - 1) / t;
        let idx = (i - t * blk) as usize;
        let k = per * blk + p[idx];
        output[(j - 1) as usize] = input[(k - 1) as usize];
    }
}

/// De-puncture `input` bits back into `output` mother‐code buffer.
pub fn tetra_rcpc_depunct(pu: RcpcPunctMode, input: &[u8], len: usize, output: &mut [u8]) {
    let puncturer = get_puncturer(pu);
    let t = puncturer.t;
    let period = puncturer.period;
    let p = puncturer.p;
    // let len = input.len() as u32;
    let len = len as u32;
    for j in 1..=len {
        let i = (puncturer.i_func)(j);
        let blk = (i - 1) / t;
        let idx = (i - t * blk) as usize;
        let k = period * blk + p[idx];
        // tracing::trace!("j = {}, i = {}, k = {}", j, i, k);
        output[(k - 1) as usize] = input[(j - 1) as usize];
    }
}

/// Compare mother vs depunct buffers, ignoring `0xff` in `depunct`.
/// Returns count of matched symbols or `Err(())` on mismatch.
pub fn mother_memcmp(mother: &[u8], depunct: &[u8]) -> Result<usize, ()> {
    let mut matched = 0;
    for (&m, &d) in mother.iter().zip(depunct.iter()) {
        if d == 0xff {
            continue;
        }
        if d != m {
            return Err(());
        }
        matched += 1;
    }
    Ok(matched)
}
