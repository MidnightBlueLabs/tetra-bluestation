pub mod removal {
    use tetra_core::bitbuffer::BitBuffer;

    /// Returns the number of fill bits at the end of the PDU in bitbuf, given the total pdu_len_bits.
    pub fn get_num_fill_bits(bitbuf: &BitBuffer, pdu_len_bits: usize, suppress_warning: bool) -> usize {
        let mut index = pdu_len_bits as isize - 1;
        // TODO FIXME improve efficiency by fetching larger chunks than 1 bit
        while index >= bitbuf.get_pos() as isize {
            let bit = bitbuf.peek_bits_startoffset(index as usize, 1).unwrap();
            if bit == 0 {
                index -= 1;
            } else {
                return (pdu_len_bits as isize - index) as usize;
            }
        }

        if !suppress_warning {
            tracing::warn!("No fill bits found");
        }

        0
    }
}

pub mod addition {
    use tetra_core::bitbuffer::BitBuffer;

    /// Compute how many fill bits need to be added in order to reach the next byte boundary
    #[inline(always)]
    pub fn compute_required_naive(total_pdu_sdu_len_bits: usize) -> usize {
        (8 - total_pdu_sdu_len_bits % 8) % 8
    }

    /// Zeroes all bits behind the current position to end of window, preceded by a 1
    pub fn write(bitbuf: &mut BitBuffer, num_fill_bits: Option<usize>) {
        if let Some(num_fill_bits) = num_fill_bits {
            if num_fill_bits > 0 {
                // Write fill bits
                bitbuf.write_bit(1);
                bitbuf.write_zeroes(num_fill_bits - 1);
            } else {
                // Zero fill bits
            }
        } else if bitbuf.get_len_remaining() > 0 {
            // Fill  buf
            bitbuf.write_bit(1);
            bitbuf.write_zeroes(bitbuf.get_len_remaining());
        } else {
            // No space left
        }
    }
}
