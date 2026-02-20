use super::dsp_types::*;

const VECLEN: usize = 4;

/// FIR filter for complex signal with symmetric real taps.
pub struct FirComplexSym {
    half_length: usize,
    i: usize,
    /// Real part of first half of history.
    /// Data is repeated twice for "fake circular buffering".
    history_re: Vec<RealSample>,
    /// Imaginary part.
    history_im: Vec<RealSample>,
    /// Real part of second half of history.
    /// The signal is reversed here to make it easier
    /// to implement a symmetric filter.
    reversed_re: Vec<RealSample>,
    /// Imaginary part.
    reversed_im: Vec<RealSample>,
}

impl FirComplexSym {
    pub fn new(half_length: usize) -> Self {
        assert!(half_length % VECLEN == 0);
        let len = half_length * 2;
        Self {
            half_length,
            i: 0,
            history_re: vec![num::zero(); len],
            history_im: vec![num::zero(); len],
            reversed_re: vec![num::zero(); len],
            reversed_im: vec![num::zero(); len],
        }
    }

    pub fn sample(&mut self, half_taps: &[RealSample], in_: ComplexSample) -> ComplexSample {
        assert!(half_taps.len() == self.half_length);
        // Index to history buffer
        let i = self.i;
        // Index to reversed history buffer
        let ir = self.half_length - 1 - i;

        // Move older samples to reversed history buffer
        self.reversed_re[ir] = self.history_re[i];
        self.reversed_re[ir + self.half_length] = self.history_re[i];
        self.reversed_im[ir] = self.history_im[i];
        self.reversed_im[ir + self.half_length] = self.history_im[i];
        // Put new sample in first history buffer
        self.history_re[i] = in_.re;
        self.history_re[i + self.half_length] = in_.re;
        self.history_im[i] = in_.im;
        self.history_im[i + self.half_length] = in_.im;

        // I tried to write the following loop so that the compiler
        // could auto-vectorize the code to use SIMD instructions.
        // Not sure if this is the best way to do it.

        let mut sum_re: [RealSample; VECLEN] = [num::zero(); VECLEN];
        let mut sum_im: [RealSample; VECLEN] = [num::zero(); VECLEN];

        for ((((t, h_re), h_im), r_re), r_im) in half_taps
            .chunks_exact(VECLEN)
            .zip(self.history_re[i + 1..i + 1 + self.half_length].chunks_exact(VECLEN))
            .zip(self.history_im[i + 1..i + 1 + self.half_length].chunks_exact(VECLEN))
            .zip(self.reversed_re[ir..ir + self.half_length].chunks_exact(VECLEN))
            .zip(self.reversed_im[ir..ir + self.half_length].chunks_exact(VECLEN))
        {
            for vi in 0..VECLEN {
                sum_re[vi] += (h_re[vi] + r_re[vi]) * t[vi];
                sum_im[vi] += (h_im[vi] + r_im[vi]) * t[vi];
            }
        }

        // Increment index
        self.i = if self.i < self.half_length - 1 { self.i + 1 } else { 0 };

        ComplexSample {
            re: sum_re.iter().sum(),
            im: sum_im.iter().sum(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_fir_complex_sym() {
        const TAPS: [RealSample; 8] = [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        let mut fir = FirComplexSym::new(TAPS.len());

        let mut out = Vec::<ComplexSample>::new();

        // Test feeding it some impulses with different values.
        // Add different numbers of zero samples in between to see that
        // buffer indexing works correctly in every case.
        let impulses_in = [
            ComplexSample { re: 1.0, im: 0.0 },
            ComplexSample { re: 0.0, im: 1.0 },
            ComplexSample { re: 0.1, im: 0.2 },
            ComplexSample { re: -0.3, im: -0.4 },
        ];
        let nzeros: [usize; 4] = [100, 101, 102, 123];
        for (in_, zeros) in impulses_in.iter().zip(nzeros) {
            out.clear();
            out.push(fir.sample(&TAPS, *in_));
            for _ in 0..zeros {
                out.push(fir.sample(&TAPS, num::zero()));
            }
            //eprintln!("{:?}", out);
            // The filter should first output values of taps reversed
            // and then not reversed, multiplied by the input value.
            // Check if the output is close enough to the expected value,
            // allowing for some rounding errors.
            fn check(value: ComplexSample, expected: ComplexSample) {
                //eprintln!("Output {}, should be {}", value, expected);
                assert!((expected.re - value.re).abs() < 1e-6);
                assert!((expected.im - value.im).abs() < 1e-6);
            }
            for i in 0..TAPS.len() {
                //eprintln!("Checking tap {}", i);
                // Reversed part of impulse response
                check(out[i], *in_ * TAPS[TAPS.len() - 1 - i]);
                // Non-reversed part
                check(out[TAPS.len() + i], in_ * TAPS[i]);
            }
            // Rest of output should be zeros
            //eprintln!("Checking output is zeros when it should be");
            for value in out[TAPS.len() * 2..].iter() {
                check(*value, num::zero());
            }
        }
    }
}
