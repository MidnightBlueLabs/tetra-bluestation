use tetra_core::BitBuffer;

/// Compute FCS checksum for a range of bits in a BitBuffer
/// Offsets are relative to the bitbuffer window start.
pub fn compute_fcs(bitbuf: &BitBuffer, start: usize, end: usize) -> u32 {
    assert!(start <= end);
    assert!(end <= bitbuf.get_len());

    let mut crc: u32 = 0xFFFFFFFF;
    let len = end - start;
    if len < 32 {
        crc <<= 32 - len;
    }

    // TODO optimize by fetching up to 64 bits per iteration
    for i in 0..len {
        let bit_pos = start + i;
        let bit = bitbuf.peek_bits_startoffset(bit_pos, 1).unwrap() as u8;
        let feedback = (bit ^ (crc >> 31) as u8) & 1;
        crc <<= 1;
        if feedback != 0 {
            crc ^= 0x04C11DB7;
        }
    }

    !crc
}

/// Computes and checks the FCS checksum
/// Computes over bitbuffer range [pos, end-32]. Checks with FCS at [end - 32, end]
pub fn check_fcs(bitbuf: &BitBuffer) -> bool {
    if bitbuf.get_len_remaining() < 32 {
        tracing::warn!(
            "check_fcs: Not enough bits for FCS, length remaining: {}",
            bitbuf.get_len_remaining()
        );
        return false;
    }
    let fcs_computed = compute_fcs(bitbuf, bitbuf.get_pos(), bitbuf.get_len() - 32);
    let fcs_extracted = bitbuf.peek_bits_startoffset(bitbuf.get_len() - 32, 32).unwrap() as u32;
    fcs_computed == fcs_extracted
}

#[cfg(test)]
mod tests {
    use tetra_pdus::llc::pdus::bl_data::BlData;

    use super::*;

    #[test]
    fn fcs_test() {
        let testvec = "010100100111101011010111110000100110000110001011000011000000000000000011000100000001001100110011000000110010001011000011001000110000001100100011000100110001001100010011000100110101001100100011000000110010001100000011000000110001011001111010000010101011000110101";
        let mut bitbuf = BitBuffer::from_bitstr(testvec);
        bitbuf.seek(5);
        let fcs = compute_fcs(&bitbuf, 5, 5 + 224);
        let extracted_fcs = bitbuf.peek_bits_startoffset(5 + 224, 32).unwrap() as u32;
        assert_eq!(fcs, extracted_fcs);
    }

    #[test]
    fn bldata_with_fcs() {
        let testvec = "010100100111101011010111110000100110000110001011000011000000000000000011000100000001001100110011000000110010001011000011001000110000001100100011000100110001001100010011000100110101001100100011000000110010001100000011000000110001011001111010000010101011000110101";
        let mut bitbuf = BitBuffer::from_bitstr(testvec);
        let pdu = BlData::from_bitbuf(&mut bitbuf).expect("Failed to parse BL-DATA PDU");
        assert!(pdu.has_fcs, "PDU should have FCS");
        let fcs_ok = check_fcs(&bitbuf);
        assert!(fcs_ok, "FCS check failed");
    }
}
