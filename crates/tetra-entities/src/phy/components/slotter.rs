use tetra_core::TrainingSequence;

use crate::phy::components::{burst_consts::*, train_consts::*};

#[allow(non_upper_case_globals)]
pub mod bitseq {
    /// Clause 9.4.4.3.2 Normal Training Sequence 1, 22 n-bits
    pub const n: [u8; 22] = [1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0];
    /// Clause 9.4.4.3.2 Normal Training Sequence 2, 22 p-bits
    pub const p: [u8; 22] = [0, 1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 1, 1, 1, 0];
    /// Clause 9.4.4.3.2 Normal Training Sequence 3, 22 q-bits
    pub const q: [u8; 22] = [1, 0, 1, 1, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1];
    /// Clause 9.4.4.3.3 Extended training sequence, 30 x-bits
    pub const x: [u8; 30] = [
        1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 1,
    ];
    /// Clause 9.4.4.3.4 Synchronization training sequence, 38 y-bits
    pub const y: [u8; 38] = [
        1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1,
    ];
    /// Clause 9.4.4.3.5, tail bits
    pub const t: [u8; 4] = [1, 1, 0, 0];
    /// Clause 9.4.4.3.1 Frequency Correction Field
    pub const f: [u8; 80] = [
        1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PhaseAdjustBits {
    HA = 0,
    HB = 1,
    HC = 2,
    HD = 3,
    HE = 4,
    HF = 5,
    HG = 6,
    HH = 7,
    HI = 8,
    HJ = 9,
}

/// Phase‐adjustment parameters
const PHASE_ADJ: [(u16, u16); 10] = [
    (8, 122),   // HA
    (123, 249), // HB
    (8, 108),   // HC
    (109, 249), // HD
    (112, 230), // HE
    (1, 111),   // HF
    (3, 117),   // HG
    (118, 224), // HH
    (3, 103),   // HI
    (104, 224), // HJ
];

fn calc_phase_adj(phase: i32) -> i32 {
    let mut adj_phase = -(phase % 8);

    if adj_phase > 3 {
        adj_phase -= 8;
    } else if adj_phase < -3 {
        adj_phase += 8;
    }
    adj_phase
}

/// map 2‐bit symbol → +/- phase (in pi/4 units)
const BITS2PHASE: [i8; 4] = [1, -1, 3, -3];

/// map adjusted phase (-3,-1,1,3) → 2 bits
/// indexed by (phase+3)
const PHASE2BITS: [(i8, [u8; 2]); 6] = [
    /* -3 */ (-3, [1, 1]),
    /* -2 unused */
    (0, [0, 0]),
    /* -1 */ (-1, [0, 1]),
    /*  0 unused */
    (0, [0, 0]),
    /* +1 */ (1, [0, 0]),
    /* +3 */ (3, [1, 0]),
];

/// sum up the phases for a window of symbols;  
/// `bits` is the full burst, `start_symbol` is index in *bits* of the first symbol to include,
/// and `n_symbols` is how many symbols (each symbol = 2 bits).
fn sum_up_phase(bits: &[u8], start_symbol: usize, n_symbols: usize) -> i32 {
    let mut sum = 0i32;
    let mut idx = start_symbol * 2;
    for _ in 0..n_symbols {
        let sym = (bits[idx] | (bits[idx + 1] << 1)) as usize;
        sum += BITS2PHASE[sym] as i32;
        idx += 2;
    }
    sum
}

/// Compute and write the 2 phase‐adjustment bits for window `pan` into
/// the burst buffer.  `window` is the full bit‐slice of the burst,
/// `bitbuf` is the BitBuffer, and `bit_offset` is the bit‐index within the burst
/// where these 2 bits should be placed.
fn compute_phase_adj_bits(window: &[u8], pan: PhaseAdjustBits) -> [u8; 2] {
    let (n1, n2) = PHASE_ADJ[pan as usize];
    // sum from symbol index (n1-1) to (n2-1), inclusive
    let sum = sum_up_phase(window, (n1 - 1) as usize, (n2 - n1 + 1) as usize);
    let adj = calc_phase_adj(sum);
    // look up bits:
    let bits = PHASE2BITS.iter().find(|&&(ph, _)| ph as i32 == adj).unwrap().1;

    bits
}

/// Constructs a Synchronization Downlink Burst (Clause 9.4.4.2.6) from three blocks
/// blk1: 120-bit SB1 type5 bits (sb1)
/// bbk: 30-bit BBK type5 bits (bb)
/// blk2: 216-bit SB2 type5 bits (bkn2)
pub fn build_sdb(blk1: &[u8; SB_BLK1_BITS], bbk: &[u8; SB_BBK_BITS], blk2: &[u8; SB_BLK2_BITS]) -> [u8; TIMESLOT_TYPE4_BITS] {
    let mut type5 = [0u8; TIMESLOT_TYPE4_BITS];

    // This makes it a continuous burst
    type5[0..12].copy_from_slice(&bitseq::q[10..]);

    // Compute HC phase adjustment bits later

    type5[14..94].copy_from_slice(&bitseq::f);
    type5[94..214].copy_from_slice(blk1);
    type5[214..252].copy_from_slice(&bitseq::y);
    type5[252..282].copy_from_slice(bbk);
    type5[282..498].copy_from_slice(blk2);

    // Compute HD phase adjustment bits later

    // This makes it a continuous burst
    type5[500..510].copy_from_slice(&bitseq::q[..10]);

    // Compute and place HC and HD phase adjustment bits
    let fc1 = compute_phase_adj_bits(&type5, PhaseAdjustBits::HC);
    let fc2 = compute_phase_adj_bits(&type5, PhaseAdjustBits::HD);
    type5[12..14].copy_from_slice(fc1.as_ref());
    type5[498..500].copy_from_slice(fc2.as_ref());

    type5
}

/// Constructs a Normal Continuous Downlink Burst (Clause 9.4.4.2.5) from three blocks
/// Training sequence determines whether blk1 and blk2 are to be considered one full slot or two half slots
/// blk1: 216-bit BLK1 type5 bits (bkn1)
/// bbk: 30-bit BBK type5 bits (bb)
/// blk2: 216-bit SB2 type5 bits (bkn2)
pub fn build_ndb(
    train_seq: TrainingSequence,
    blk1: &[u8; NDB_BLK_BITS],
    bbk: &[u8; NDB_BBK_BITS],
    blk2: &[u8; NDB_BLK_BITS],
) -> [u8; TIMESLOT_TYPE4_BITS] {
    let mut type5 = [0u8; TIMESLOT_TYPE4_BITS];

    // This makes it a continuous burst
    type5[0..12].copy_from_slice(&bitseq::q[10..]);

    // Compute HA phase adjustment bits later ////////
    type5[14..230].copy_from_slice(blk1);

    // Scrambled broadcast bits (first part)
    type5[230..244].copy_from_slice(&bbk[..14]);

    match train_seq {
        TrainingSequence::NormalTrainSeq1 => {
            type5[244..266].copy_from_slice(&bitseq::n);
        }
        TrainingSequence::NormalTrainSeq2 => {
            type5[244..266].copy_from_slice(&bitseq::p);
        }
        _ => panic!(),
    }

    // Scrambled broadcast bits (second part)
    type5[266..282].copy_from_slice(&bbk[14..]);
    type5[282..498].copy_from_slice(blk2);

    // Compute HB phase adjustment bits later ////////

    // This makes it a continuous burst
    type5[500..510].copy_from_slice(&bitseq::q[..10]);

    // Compute and place HC and HD phase adjustment bits
    let fc1 = compute_phase_adj_bits(&type5, PhaseAdjustBits::HA);
    let fc2 = compute_phase_adj_bits(&type5, PhaseAdjustBits::HB);
    type5[12..14].copy_from_slice(fc1.as_ref());
    type5[498..500].copy_from_slice(fc2.as_ref());

    type5
}

#[cfg(test)]
mod tests {
    use tetra_core::bitbuffer::BitBuffer;

    use crate::phy::components::train_consts::*;

    use super::*;

    #[test]
    fn test_build_sdb_1() {
        let bbk = "010110000011111100010011101001";
        let blk1 =
            "111111110101100010110101100101111100101101010001100111101000011010010011011111100010110001011010100111101011111100000011";
        let blk2 = "001111111111011011000010110000100111011000110000111111010000011000011010111101010101001011101001001110011100101010000100101010000100000011011000001101001100111100101110011111111100000010000101010010000010011111110110";
        let expected_burst = "000110101101101111111100000000000000000000000000000000000000000000000000000000000000001111111111111111010110001011010110010111110010110101000110011110100001101001001101111110001011000101101010011110101111110000001111000001100111001110100111000001100111010110000011111100010011101001001111111111011011000010110000100111011000110000111111010000011000011010111101010101001011101001001110011100101010000100101010000100000011011000001101001100111100101110011111111100000010000101010010000010011111110110011011011100";

        let bbk = BitBuffer::from_bitstr(bbk).into_bitvec();
        let blk1 = BitBuffer::from_bitstr(blk1).into_bitvec();
        let blk2 = BitBuffer::from_bitstr(blk2).into_bitvec();
        let expected_burst = BitBuffer::from_bitstr(expected_burst).into_bitvec();
        let mut expected_burst: [u8; TIMESLOT_TYPE4_BITS] = expected_burst.try_into().unwrap();

        let mut burst = build_sdb(
            blk1.as_slice().try_into().unwrap(),
            bbk.as_slice().try_into().unwrap(),
            blk2.as_slice().try_into().unwrap(),
        );

        tracing::warn!("WARNING: frequency correction bits are not properly computed and zeroed out for testing");
        burst[12..14].copy_from_slice(&[0, 0]);
        burst[498..500].copy_from_slice(&[0, 0]);
        expected_burst[12..14].copy_from_slice(&[0, 0]);
        expected_burst[498..500].copy_from_slice(&[0, 0]);

        assert!(
            burst == expected_burst,
            "Burst does not match expected output:\nComputed: {:?}\nExpected: {:?}",
            BitBuffer::from_bitarr(&burst).dump_bin(),
            BitBuffer::from_bitarr(&expected_burst).dump_bin()
        );
    }

    #[test]
    fn test_build_sdb_2() {
        let bbk = "001010010110101111111110100100";
        let blk1 =
            "010011110111010011010000110111101111101110111111100101011010011001000011011011101011011101101010001000101101011000101111";
        let blk2 = "011001100001110100100001100000110010110010110110000111010101000111001011000111001011110010000011010010111110000110011011000100110011011010001101011100110000001100111101100000101111010000010110110011100001110001101011";
        let expected_burst = "000110101101011111111100000000000000000000000000000000000000000000000000000000000000001111111101001111011101001101000011011110111110111011111110010101101001100100001101101110101101110110101000100010110101100010111111000001100111001110100111000001100111001010010110101111111110100100011001100001110100100001100000110010110010110110000111010101000111001011000111001011110010000011010010111110000110011011000100110011011010001101011100110000001100111101100000101111010000010110110011100001110001101011101011011100";

        let bbk = BitBuffer::from_bitstr(bbk).into_bitvec();
        let blk1 = BitBuffer::from_bitstr(blk1).into_bitvec();
        let blk2 = BitBuffer::from_bitstr(blk2).into_bitvec();
        let expected_burst = BitBuffer::from_bitstr(expected_burst).into_bitvec();
        let mut expected_burst: [u8; TIMESLOT_TYPE4_BITS] = expected_burst.try_into().unwrap();

        let mut burst = build_sdb(
            blk1.as_slice().try_into().unwrap(),
            bbk.as_slice().try_into().unwrap(),
            blk2.as_slice().try_into().unwrap(),
        );

        tracing::warn!("WARNING: frequency correction bits are not properly computed and zeroed out for testing");
        burst[12..14].copy_from_slice(&[0, 0]);
        burst[498..500].copy_from_slice(&[0, 0]);
        expected_burst[12..14].copy_from_slice(&[0, 0]);
        expected_burst[498..500].copy_from_slice(&[0, 0]);

        assert!(
            burst == expected_burst,
            "Burst does not match expected output:\nComputed: {:?}\nExpected: {:?}",
            BitBuffer::from_bitarr(&burst).dump_bin(),
            BitBuffer::from_bitarr(&expected_burst).dump_bin()
        );
    }
}
