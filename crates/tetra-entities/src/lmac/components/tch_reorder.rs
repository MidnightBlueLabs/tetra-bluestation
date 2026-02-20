/// TETRA ACELP bit reordering: converts between codec order (STE format) and channel order (type-1 bits).
///
/// TCH/S carries two 137-bit subframes (274 bits total), interleaved by sensitivity class (EN 300 395-2, Table 4):
///   - Class 0: 51 bits/subframe (unprotected)
///   - Class 1: 56 bits/subframe (medium)
///   - Class 2: 30 bits/subframe (most sensitive)

const NUM_ACELP_BITS: usize = 137; // bits per subframe

/// Class 0 positions (1-indexed within 137-bit subframe) from EN 300 395-2, Table 4.
const CLASS0_POS: [u8; 51] = [
    35, 36, 37, 38, 39, 40, 41, 42, 43, 47, 48, 56, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 74, 75, 83, 88, 89, 90, 91, 92, 93, 94, 95, 96,
    97, 101, 102, 110, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 128, 129, 137,
];

/// EN 300 395-2, Table 4 — Class 1 positions (1-indexed)
const CLASS1_POS: [u8; 56] = [
    58, 85, 112, 54, 81, 108, 135, 50, 77, 104, 131, 45, 72, 99, 126, 55, 82, 109, 136, 5, 13, 34, 8, 16, 17, 22, 23, 24, 25, 26, 6, 14, 7,
    15, 60, 87, 114, 46, 73, 100, 127, 44, 71, 98, 125, 33, 49, 76, 103, 130, 59, 86, 113, 57, 84, 111,
];

/// EN 300 395-2, Table 4 — Class 2 positions (1-indexed)
const CLASS2_POS: [u8; 30] = [
    18, 19, 20, 21, 31, 32, 53, 80, 107, 134, 1, 2, 3, 4, 9, 10, 11, 12, 27, 28, 29, 30, 52, 79, 106, 133, 51, 78, 105, 132,
];

/// Convert 274 ACELP bits from codec order (STE format) to channel order (type-1 bits). One-bit-per-byte.
pub fn codec_to_channel(codec_bits: &[u8; 274]) -> [u8; 274] {
    let mut channel = [0u8; 274];
    let mut out_idx = 0;

    // Class 0: 51 bits per subframe, interleaved between subframes
    for bit in 0..CLASS0_POS.len() {
        let pos = (CLASS0_POS[bit] - 1) as usize; // 0-indexed within subframe
        channel[out_idx] = codec_bits[0 * NUM_ACELP_BITS + pos]; // subframe 0
        channel[out_idx + 1] = codec_bits[1 * NUM_ACELP_BITS + pos]; // subframe 1
        out_idx += 2;
    }

    // Class 1: 56 bits per subframe
    for bit in 0..CLASS1_POS.len() {
        let pos = (CLASS1_POS[bit] - 1) as usize;
        channel[out_idx] = codec_bits[0 * NUM_ACELP_BITS + pos];
        channel[out_idx + 1] = codec_bits[1 * NUM_ACELP_BITS + pos];
        out_idx += 2;
    }

    // Class 2: 30 bits per subframe
    for bit in 0..CLASS2_POS.len() {
        let pos = (CLASS2_POS[bit] - 1) as usize;
        channel[out_idx] = codec_bits[0 * NUM_ACELP_BITS + pos];
        channel[out_idx + 1] = codec_bits[1 * NUM_ACELP_BITS + pos];
        out_idx += 2;
    }

    debug_assert_eq!(out_idx, 274);
    channel
}

/// Convert 274 ACELP bits from channel order (type-1/type-2 bits) to codec order (STE format). Reverse of `codec_to_channel`.
pub fn channel_to_codec(channel_bits: &[u8; 274]) -> [u8; 274] {
    let mut codec = [0u8; 274];
    let mut in_idx = 0;

    // Class 0
    for bit in 0..CLASS0_POS.len() {
        let pos = (CLASS0_POS[bit] - 1) as usize;
        codec[0 * NUM_ACELP_BITS + pos] = channel_bits[in_idx]; // subframe 0
        codec[1 * NUM_ACELP_BITS + pos] = channel_bits[in_idx + 1]; // subframe 1
        in_idx += 2;
    }

    // Class 1
    for bit in 0..CLASS1_POS.len() {
        let pos = (CLASS1_POS[bit] - 1) as usize;
        codec[0 * NUM_ACELP_BITS + pos] = channel_bits[in_idx];
        codec[1 * NUM_ACELP_BITS + pos] = channel_bits[in_idx + 1];
        in_idx += 2;
    }

    // Class 2
    for bit in 0..CLASS2_POS.len() {
        let pos = (CLASS2_POS[bit] - 1) as usize;
        codec[0 * NUM_ACELP_BITS + pos] = channel_bits[in_idx];
        codec[1 * NUM_ACELP_BITS + pos] = channel_bits[in_idx + 1];
        in_idx += 2;
    }

    debug_assert_eq!(in_idx, 274);
    codec
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_codec_channel_codec() {
        // Create a test pattern where each bit position has a unique value
        let mut codec_bits = [0u8; 274];
        for i in 0..274 {
            codec_bits[i] = (i % 2) as u8;
        }

        let channel = codec_to_channel(&codec_bits);
        let recovered = channel_to_codec(&channel);

        assert_eq!(codec_bits, recovered, "roundtrip codec→channel→codec failed");
    }

    #[test]
    fn reorder_changes_bits() {
        // Verify that reordering actually changes the order (not identity)
        let mut codec_bits = [0u8; 274];
        for i in 0..274 {
            codec_bits[i] = ((i * 7 + 3) % 2) as u8; // pseudo-random pattern
        }

        let channel = codec_to_channel(&codec_bits);
        assert_ne!(codec_bits[..], channel[..], "reordering should change the bit order");
    }

    #[test]
    fn position_tables_cover_all_bits() {
        // Verify that every bit position 1-137 is covered exactly once
        let mut covered = [false; 137];
        for &p in CLASS0_POS.iter().chain(CLASS1_POS.iter()).chain(CLASS2_POS.iter()) {
            let idx = (p - 1) as usize;
            assert!(!covered[idx], "position {} is duplicated", p);
            covered[idx] = true;
        }
        for (i, &c) in covered.iter().enumerate() {
            assert!(c, "position {} is not covered", i + 1);
        }
    }
}
