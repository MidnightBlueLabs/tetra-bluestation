/// Generator matrix from Section 8.2.3.2
pub const RM_30_14_GEN: [[u8; 16]; 14] = [
    [1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0],
    [0, 0, 1, 0, 1, 1, 0, 1, 1, 1, 1, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    [1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0],
    [1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 1, 1, 1, 0, 1, 0],
    [0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0],
    [0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1],
    [1, 0, 0, 0, 0, 0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1],
    [0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 0, 1, 0, 1],
    [0, 0, 1, 0, 0, 0, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1],
    [0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1],
    [0, 0, 0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1],
    [0, 0, 0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 1, 1, 1],
];

/// Static array with precomputed row masks
pub static RM_30_14_ROWS_PRECOMPUTED: [u32; 14] = [
    0x20009b60, 0x10002de0, 0x0800fc20, 0x0400e03c, 0x0200983a, 0x01005436, 0x00802c2e, 0x0040ffdf, 0x00208339, 0x001042b5, 0x000821ad,
    0x00041273, 0x0002096b, 0x000104e7,
];

/// Compute RM(30,14) codeword for a 14-bit input (upper 14 bits of codeword)
pub fn tetra_rm3014_compute(input: u16) -> u32 {
    let mut val = 0u32;

    for i in 0..14 {
        let bit = (input >> (13 - i)) & 1;
        if bit == 1 {
            val ^= RM_30_14_ROWS_PRECOMPUTED[i];
        }
    }
    val
}

/// "Decode" systematic RM(30,14): extract original 14-bit data
/// Does not perform error correction, just extracts the upper 14 bits
pub fn tetra_rm3014_decode_naive(codeword: u32) -> u16 {
    (codeword >> 16) as u16
}

/// Compute column‐syndromes for single‐error decoding
pub const fn compute_col_syndromes() -> [u16; 30] {
    let mut out = [0u16; 30];
    let mut k = 0;
    while k < 30 {
        let mut syn = 0u16;
        let mut j = 0;
        while j < 16 {
            let bit = if k < 14 {
                RM_30_14_GEN[k][j] as u16
            } else {
                ((k - 14) == j) as u16
            };
            syn |= bit << j;
            j += 1;
        }
        out[k] = syn;
        k += 1;
    }
    out
}

pub const COL_SYNDROMES: [u16; 30] = compute_col_syndromes();

/// Quick and dirty single-bit error correction
/// Compute syndrome of a 30‐bit codeword
pub fn compute_syndrome(codeword: u32) -> u16 {
    let mut syn = 0u16;
    let mut j = 0;
    while j < 16 {
        let mut sum = 0u8;
        let mut i = 0;
        while i < 14 {
            sum ^= ((codeword >> (29 - i)) & 1) as u8 & RM_30_14_GEN[i][j];
            i += 1;
        }
        sum ^= ((codeword >> (29 - (14 + j))) & 1) as u8;
        if sum & 1 != 0 {
            syn |= 1 << j;
        }
        j += 1;
    }
    syn
}

pub fn tetra_rm3014_decode_limited_ecc(codeword: u32) -> u16 {
    let syn = compute_syndrome(codeword);
    let mut corrected = codeword;
    if syn != 0 {
        for (k, &col_syn) in COL_SYNDROMES.iter().enumerate() {
            if col_syn == syn {
                corrected ^= 1 << (29 - k);
                break;
            }
        }
    }
    (corrected >> 16) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_no_error() {
        let messages = [0u16, 1u16, 0x1FFFu16, 0x1234u16, 0x2A3Bu16];
        for &msg in &messages {
            let code = tetra_rm3014_compute(msg);
            assert_eq!(tetra_rm3014_decode_naive(code), msg);
            assert_eq!(tetra_rm3014_decode_limited_ecc(code), msg);
        }
    }

    #[test]
    fn test_single_bit_error_correction() {
        let messages = [0u16, 1u16, 0x1FFFu16, 0x1234u16, 0x2A3Bu16];

        for &msg in &messages {
            let code = tetra_rm3014_compute(msg);
            for bit in 0..30 {
                let erroneous = code ^ (1 << bit);
                let decoded = tetra_rm3014_decode_limited_ecc(erroneous);
                assert_eq!(decoded, msg, "Failed to correct bit {}", bit);
            }
        }
    }

    #[test]
    fn test_uncorrectable_errors() {
        let msg = 0x1234u16;
        let code = tetra_rm3014_compute(msg);
        let erroneous = code ^ 0xbadd00;
        let decoded = tetra_rm3014_decode_limited_ecc(erroneous);
        assert_ne!(decoded, msg);
    }
}
