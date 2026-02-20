pub const fn block_interl_func(k: u32, a: u32, i: u32) -> u32 {
    1 + ((a.wrapping_mul(i)) % k)
}

pub fn block_interleave(k: usize, a: usize, input: &[u8], output: &mut [u8]) {
    assert!(input.len() >= k && output.len() >= k);
    for i in 1..=k {
        let k = block_interl_func(k as u32, a as u32, i as u32) as usize;
        output[k - 1] = input[i - 1];
    }
}

pub fn block_deinterleave(k: usize, a: usize, input: &[u8], output: &mut [u8]) {
    assert!(input.len() >= k && output.len() >= k);
    for i in 1..=k {
        let k = block_interl_func(k as u32, a as u32, i as u32) as usize;
        output[i - 1] = input[k - 1];
    }
}

pub fn matrix_interleave(lines: usize, columns: usize, input: &[u8], output: &mut [u8]) {
    let total = lines.checked_mul(columns).expect("overflow");
    assert!(input.len() >= total && output.len() >= total);
    for i in 0..columns {
        for j in 0..lines {
            output[i * lines + j] = input[j * columns + i];
        }
    }
}

pub fn matrix_deinterleave(lines: usize, columns: usize, input: &[u8], output: &mut [u8]) {
    let total = lines.checked_mul(columns).expect("overflow");
    assert!(input.len() >= total && output.len() >= total);
    for i in 0..columns {
        for j in 0..lines {
            output[j * columns + i] = input[i * lines + j];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_interleave_roundtrip() {
        let k = 10;
        let a = 3;
        let data: Vec<u8> = (0..k as u8).collect();
        let mut tmp = vec![0u8; k];
        let mut out = vec![0u8; k];

        block_interleave(k, a, &data, &mut tmp);
        block_deinterleave(k, a, &tmp, &mut out);
        assert_eq!(data, out);
    }

    #[test]
    fn test_matrix_interleave_roundtrip() {
        let lines = 4;
        let columns = 3;
        let data: Vec<u8> = (0..(lines * columns) as u8).collect();
        let mut tmp = vec![0u8; lines * columns];
        let mut out = vec![0u8; lines * columns];

        matrix_interleave(lines, columns, &data, &mut tmp);
        matrix_deinterleave(lines, columns, &tmp, &mut out);
        assert_eq!(data, out);
    }
}
