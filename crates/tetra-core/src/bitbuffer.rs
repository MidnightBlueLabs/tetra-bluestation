use std::{
    cmp::{max, min},
    fmt,
};

use crate::pdu_parse_error::PduParseErr;

pub struct BitBuffer {
    buffer: Vec<u8>,
    start: usize,          // bits before this are out of window
    pos: usize,            // next bit offset for read/write (absolute)
    end: usize,            // bits at or after this are out of window
    flag_autoexpand: bool, // if true, ignores end pointer on writes and reallocates buffer if insufficient capacity
}

impl BitBuffer {
    /// Create a zeroed buffer capable of holding exactly `len_bits` bits.
    pub fn new(len_bits: usize) -> Self {
        let byte_len = (len_bits + 7) / 8;
        BitBuffer {
            buffer: vec![0; byte_len],
            start: 0,
            pos: 0,
            end: len_bits,
            flag_autoexpand: false,
        }
    }

    /// Create a zeroed buffer with an inital capacity but zero length (end is set to 0).
    /// Writes to this buffer will automatically advance the end pointer and reallocate the buffer if needed
    pub fn new_autoexpand(initial_max_len_bits: usize) -> Self {
        let byte_len = (initial_max_len_bits + 7) / 8;
        BitBuffer {
            buffer: vec![0; byte_len],
            start: 0,
            pos: 0,
            end: 0,
            flag_autoexpand: true,
        }
    }

    /// Wrap an existing byte-vector as a BitBuffer (all bits initially readable/writeable).
    /// No new allocation is needed here.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let len_bits = data.len() * 8;
        BitBuffer {
            buffer: data,
            start: 0,
            pos: 0,
            end: len_bits,
            flag_autoexpand: false,
        }
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        let len_bits = data.len() * 8;
        BitBuffer {
            buffer: data.to_vec(),
            start: 0,
            pos: 0,
            end: len_bits,
            flag_autoexpand: false,
        }
    }

    /// Determines whether the buffer is expanded when a write is done that goes beyond the capacity of the buffer
    /// WARNING: Best to use BitBuffer::new_autoexpand() instead of calling this method, as the end pointer behaves
    /// differently.
    pub fn set_auto_expand(mut self, do_auto_expand: bool) {
        self.flag_autoexpand = do_auto_expand;
    }

    /// Construct a BitBuffer directly from a string of '0'/'1' characters.
    /// Panics if any other character is encountered.
    pub fn from_bitstr(bitstr: &str) -> Self {
        let len = bitstr.len();
        let mut buf = BitBuffer::new(len);
        for c in bitstr.chars() {
            match c {
                '0' => buf.write_bit(0),
                '1' => buf.write_bit(1),
                other => panic!("from_bitstr: invalid character `{}`; only '0' or '1' allowed", other),
            }
        }
        // reset pos back to start
        buf.pos = buf.start;
        buf
    }

    /// Construct a BitBuffer directly from a byte array of '0'/'1' bytes.
    pub fn from_bitarr(data: &[u8]) -> Self {
        let mut buf = BitBuffer::new(data.len());
        buf.copy_bits_from_bitarr(data);

        // Reset pos back to start
        buf.pos = buf.start;

        buf
    }

    /// Construct a BitBuffer from another bitbuffer, starting at `start` and ending at `end`.
    /// The copy will be performed efficiently as bytes, as such, `start` may not be 0 and heading/trailing data may be present.
    pub fn from_bitbuffer(bitbuffer: &BitBuffer) -> Self {
        // Compute how many bits to copy when we want to perform byte-wise copy.
        let start_byte = bitbuffer.start / 8;
        let end_byte = (bitbuffer.end + 7) / 8;
        let alloc_bits = (end_byte - start_byte) * 8;

        // Allocate new BitBuffer, copy data, and set offsets
        let mut buf = BitBuffer::new(alloc_bits);
        buf.buffer.copy_from_slice(&bitbuffer.buffer[start_byte..end_byte]);
        buf.start = bitbuffer.start % 8;
        buf.pos = buf.start;
        buf.end = buf.start + bitbuffer.get_len();
        buf
    }

    /// Construct a BitBuffer from another bitbuffer, starting at `pos` and ending at `end`.
    /// The copy will be performed efficiently as bytes, as such, `start` may not be 0 and heading/trailing data may be present.
    pub fn from_bitbuffer_pos(bitbuffer: &BitBuffer) -> Self {
        // Compute how many bits to copy when we want to perform byte-wise copy.
        let start_byte = bitbuffer.pos / 8;
        let end_byte = (bitbuffer.end + 7) / 8;
        let alloc_bits = (end_byte - start_byte) * 8;

        // Allocate new BitBuffer, copy data, and set offsets
        let mut buf = BitBuffer::new(alloc_bits);
        buf.buffer.copy_from_slice(&bitbuffer.buffer[start_byte..end_byte]);
        buf.start = bitbuffer.pos % 8;
        buf.pos = buf.start;
        buf.end = buf.start + bitbuffer.get_len_remaining();
        buf
    }

    /// Takes slice as parameter for output. Reads slice.len() bits from bitbuf[pos], and writes to output slice. 1 bit per byte.
    pub fn to_bitarr(&mut self, buf: &mut [u8]) {
        // TODO bounds check here, optimize performance
        let num_bits = buf.len();
        let mut bits_remaining = num_bits;
        while bits_remaining > 0 {
            let bits_to_read = usize::min(bits_remaining, 64);
            let v = self.read_bits(bits_to_read).unwrap(); // Guaranteed
            for i in 0..bits_to_read {
                let bit = ((v >> (bits_to_read - 1 - i)) & 0x1) as u8;
                buf[buf.len() - bits_remaining + i] = bit;
            }
            bits_remaining -= bits_to_read;
        }
    }

    /// Convert the entire window (start to end) into a String of '0'/'1' characters.
    pub fn to_bitstr(&self) -> String {
        let mut s = String::with_capacity(self.get_len());
        for i in 0..self.get_len() {
            let bit = self.read_bit_at_unchecked(self.start + i);
            s.push(if bit == 1 { '1' } else { '0' });
        }
        s
    }

    /// Peek `num_bits` at the current pos, without advancing.
    /// Returns None on overflow or if `num_bits>64`.
    pub fn peek_bits(&self, num_bits: usize) -> Option<u64> {
        // tracing::trace!("peek_bits: {} bits", num_bits);
        self.peek_bits_posoffset(0, num_bits)
    }

    /// Peek `num_bits` at the current pos with signed offset, without advancing.
    /// Returns None on overflow or if `num_bits>64`.
    pub fn peek_bits_posoffset(&self, offset: isize, num_bits: usize) -> Option<u64> {
        // let abs_offset = self.pos as isize + offset;
        // tracing::trace!("peek_bits_posoffset: {} bits at {}", num_bits, self.pos as isize + offset);
        // println!("peek_bits_offset: {} bits at {}", num_bits, self.pos as isize + offset);
        let start_offset = self.pos - self.start + offset as usize;
        self.peek_bits_startoffset(start_offset, num_bits)
    }

    /// Peek `num_bits` with offset from window start, without advancing.
    /// Returns None on overflow or if `num_bits>64`.
    pub fn peek_bits_startoffset(&self, offset: usize, num_bits: usize) -> Option<u64> {
        let abs_pos = self.start + offset;
        if num_bits > 64 || self.start + offset + num_bits > self.end {
            return None;
        }

        // tracing::debug!("peek_bits_startoffset: <{} ^{} >{}, peek[{}..{}] ({} bits)", self.start, self.pos, self.end, abs_pos, abs_pos + num_bits, num_bits);
        Some(self.read_bits_at_unchecked(abs_pos, num_bits))
    }

    /// Read `num_bits` at the current pos, advancing on success.
    pub fn read_bits(&mut self, num_bits: usize) -> Option<u64> {
        let v = self.peek_bits_startoffset(self.pos - self.start, num_bits)?;
        self.pos += num_bits;
        Some(v)
    }

    /// Similar to read_bits, but returns a ParseError::BufferEnded with the given error_string if not enough bits are available.
    pub fn read_field(&mut self, num_bits: usize, error_string: &'static str) -> Result<u64, PduParseErr> {
        self.read_bits(num_bits)
            .ok_or(PduParseErr::BufferEnded { field: Some(error_string) })
    }

    pub fn read_bit(&mut self) -> Option<u8> {
        let v = self.peek_bits_startoffset(self.pos - self.start, 1)?;
        self.pos += 1;
        Some(v as u8)
    }

    fn _realloc_tail(&mut self, new_cap_bits: usize) {
        let new_cap_bytes = (new_cap_bits + 7) / 8;
        assert!(
            new_cap_bytes >= self.buffer.len(),
            "new capacity must be larger than current buffer size"
        );

        // tracing::info!("BitBuffer: reallocating to {} bits {} bytes", new_cap_bits, new_cap_bytes);
        self.buffer.resize(new_cap_bytes, 0);
    }

    // fn _realloc_add_head(&mut self, new_cap_bits: usize) {
    //     assert!(false, "untested");
    //     let new_cap_bytes = (new_cap_bits + 7) / 8;
    //     self.buffer.reserve(new_cap_bytes);
    //     let len = self.buffer.len();
    //     unsafe {
    //         let ptr = self.buffer.as_mut_ptr();
    //         std::ptr::copy(ptr, ptr.add(new_cap_bytes), len);
    //         for i in 0..new_cap_bytes {
    //             ptr.add(i).write(0);
    //         }
    //         self.buffer.set_len(len + new_cap_bytes);
    //     }
    // }

    /// When a write would exceed the end, but the BitBuffer is set to automatically expand,
    /// this function is called to increase `end` and if needed, allocate more space in the buffer.
    fn _move_end(&mut self, needed_extra_bits: usize) {
        // Check if realloc needed, perform if needed
        let free_cap_bits = self.buffer.len() * 8 - self.end;
        let needed_total_bits = self.end + needed_extra_bits;

        if needed_extra_bits > free_cap_bits {
            let double_cap_bits = self.buffer.len() * 8 * 2;
            let new_cap_bits = max(needed_total_bits, double_cap_bits);

            // Reallocate buffer to at least `end` bits
            // tracing::info!("BitBuffer: reallocating to {} bits", new_cap_bits);
            self._realloc_tail(new_cap_bits);
        }

        self.end += needed_extra_bits;
    }

    /// Xor the next bit (at pos) with value (0 or 1)
    pub fn xor_bit(&mut self, value: u8) {
        let index = self.pos / 8;
        self.buffer[index] ^= value << (7 - (self.pos % 8)) as u8;
        self.pos += 1;
    }

    /// Xors bytearray into a bitbuffer, starting at pos, for num_bits.
    /// Advances pos by num_bits.
    pub fn xor_bytearr(&mut self, data: &[u8], num_bits: usize) -> Option<()> {
        let mut bits_remaining = num_bits;
        let mut data_pos = 0;
        while bits_remaining > 0 {
            // Read chunk of bits
            let chunk_bits = usize::min(bits_remaining, 64);
            bits_remaining -= chunk_bits;
            let val = self.read_bits(chunk_bits)?;
            // println!("xor_bytearr: read chunk_bits {}, val {:X}", chunk_bits, val);

            // Convert bytes from data array to u64 and xor with read val
            let mut xor_data = 0u64;
            let db_chunk_bytes = (chunk_bits + 7) / 8;
            for _ in 0..db_chunk_bytes {
                xor_data <<= 8;
                xor_data |= data[data_pos] as u64;
                data_pos += 1;
            }
            xor_data >>= (8 - (chunk_bits % 8)) % 8; // shift back if last byte is not fully applied
            let xorred_data = val ^ xor_data;

            // println!("xor_bytearr: chunk_bits {}, val {:X}, xor_data {:X}, xorred_data {:X}", chunk_bits, val, xor_data, xorred_data);

            // Seek back to where we were before reading
            self.seek_rel(-1 * chunk_bits as isize);
            self.write_bits(xorred_data, chunk_bits);
        }
        Some(())
    }

    /// Write a single bit to pos
    pub fn write_bit(&mut self, value: u8) {
        assert!(value == 0 || value == 1, "write_bit: value must be 0 or 1");
        if self.pos + 1 > self.end {
            if self.flag_autoexpand {
                // Advance end pointer and, realloc if needed
                self._move_end(1);
            } else {
                assert!(false, "write_bit would exceed buffer end");
            }
        }

        let index = self.pos / 8;
        let mask = 1 << (7 - (self.pos % 8)) as u8;

        self.buffer[index] &= !mask;
        self.buffer[index] |= value << (7 - (self.pos % 8)) as u8;
        self.pos += 1;
    }

    /// Write an arbitrary amount of zero-bits
    pub fn write_zeroes(&mut self, num_bits: usize) {
        let mut bits_remaining = num_bits;
        while bits_remaining > 0 {
            let chunk_size = min(bits_remaining, 64);
            self.write_bits(0, chunk_size);
            bits_remaining -= chunk_size;
        }
    }

    /// Write an arbitrary amount of one-bits
    pub fn write_ones(&mut self, num_bits: usize) {
        let mut bits_remaining = num_bits;
        while bits_remaining > 0 {
            let chunk_size = min(bits_remaining, 64);
            let val = 0xFFFFFFFFFFFFFFFF >> (64 - chunk_size);
            self.write_bits(val, chunk_size);
            bits_remaining -= chunk_size;
        }
    }

    /// Write up to 64 bits, advancing pos.
    /// If autoexpand is enabled, will advance end as well and/or realloc if buffer full
    /// If disables, panics if exceeds end.
    pub fn write_bits(&mut self, value: u64, num_bits: usize) {
        // tracing::debug!("write_bits: <{} ^{} >{}, write[{}..{}] ({} bits)", self.start, self.pos, self.end, self.pos, self.pos + num_bits, num_bits);
        assert!(num_bits <= 64, "can only write up to 64 bits");
        assert!(
            num_bits == 64 || value >> num_bits == 0,
            "value exceeds num_bits {} {}",
            value,
            num_bits
        );

        // Check if exceeding end, to either fail or expand
        if self.pos + num_bits > self.end {
            if self.flag_autoexpand {
                self._move_end(num_bits);
            } else {
                assert!(false, "write would exceed buffer end");
            }
        }

        let mut remaining = num_bits;
        let mut cur = self.pos;

        // mask to lower `num_bits` bits
        let v = if num_bits == 64 { value } else { value & ((1 << num_bits) - 1) };

        // 1) head bits
        let head_offset = cur % 8;
        if head_offset != 0 && remaining > 0 {
            let h = usize::min(remaining, 8 - head_offset);
            let idx = cur / 8;
            let byte = &mut self.buffer[idx];
            // top h bits of v:
            let bits_to_write = ((v >> (remaining - h)) as u8) & ((1 << h) - 1);
            let shift = 8 - (head_offset + h);
            let mask = ((1 << h) - 1) << shift;
            *byte = (*byte & !(mask as u8)) | (bits_to_write << shift);
            cur += h;
            remaining -= h;
        }

        // 2) full bytes
        while remaining >= 8 {
            let idx = cur / 8;
            let byte_val = ((v >> (remaining - 8)) & 0xFF) as u8;
            self.buffer[idx] = byte_val;
            cur += 8;
            remaining -= 8;
        }

        // 3) tail bits
        if remaining > 0 {
            let idx = cur / 8;
            let byte = &mut self.buffer[idx];
            let bits_to_write = (v as u8) & ((1 << remaining) - 1);
            let shift = 8 - (cur % 8 + remaining);
            let mask = ((1 << remaining) - 1) << shift;
            *byte = (*byte & !(mask as u8)) | (bits_to_write << shift);
            // cur += remaining;
            // remaining = 0;
        }

        // advance absolute position by full `num_bits`
        self.pos += num_bits;
    }

    /// Read `num_bits` from a source bitbuffer, starting at `pos`.
    /// Write this data into the current bitbuffer at the current `pos`.
    pub fn copy_bits(&mut self, src_bitbuf: &mut BitBuffer, num_bits: usize) {
        let mut bits_remaining = num_bits;
        while bits_remaining > 0 {
            let bits_to_copy = usize::min(bits_remaining, 64);
            assert!(
                src_bitbuf.get_len_remaining() >= bits_to_copy,
                "not enough bits remaining in source buffer to copy"
            );
            let v = src_bitbuf.read_bits(bits_to_copy).unwrap(); // Guaranteed
            self.write_bits(v, bits_to_copy);
            bits_remaining -= bits_to_copy;
        }
    }

    pub fn copy_bits_from_bitarr(&mut self, buf: &[u8]) {
        // TODO optimize performance
        for i in 0..buf.len() {
            let bit = buf[i];
            assert!(
                bit == 0 || bit == 1,
                "copy_bits_from_bitarr: invalid byte `{}`; only '0' or '1' allowed",
                bit
            );
            self.write_bit(bit);
        }
    }

    /// Extract the internal byte-vector (all bytes, including any unused bits).
    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    /// Convert entire window (start to end) into an array with 0 or 1 value per byte
    pub fn into_bitvec(self) -> Vec<u8> {
        let mut ret = Vec::with_capacity(self.get_len());
        for i in 0..self.get_len() {
            ret.push(self.read_bit_at_unchecked(i));
        }
        ret
    }

    /// Active window length (bits), from start to end
    pub fn get_len(&self) -> usize {
        self.end - self.start
    }

    /// Number of bits left in the window (bits), from pos to end.
    pub fn get_len_remaining(&self) -> usize {
        self.end - self.pos
    }

    /// Number of bits written, from start to pos.
    pub fn get_len_written(&self) -> usize {
        self.pos - self.start
    }

    /// Get the current position, relative to window
    pub fn get_pos(&self) -> usize {
        self.pos - self.start
    }

    /// Seek `pos` to `offset` (relative to window start).
    pub fn seek(&mut self, offset: usize) {
        let abs = self.start + offset;
        assert!(
            abs <= self.end,
            "seek out of window: got {}, allowed [{},{}]",
            abs,
            self.start,
            self.end
        );
        self.pos = abs;
    }

    /// Move the current bit‐pointer by `offset` bits (can be negative).
    /// Panics if the resulting position would lie outside the window `[start..=end]`.
    pub fn seek_rel(&mut self, offset: isize) {
        let new_pos = (self.pos as isize + offset) as usize;
        assert!(
            new_pos >= self.start && new_pos <= self.end,
            "seek out of window: got {}, allowed [{},{}]",
            new_pos,
            self.start,
            self.end
        );
        self.pos = new_pos;
    }

    // Raw operations that do not take start, end cursors into account ///////////////////////////////////////

    /// Get absolute value of window start
    pub fn get_raw_start(&self) -> usize {
        self.start
    }
    /// Get absolute value of window end, disregarding start of window
    pub fn get_raw_end(&self) -> usize {
        self.end
    }
    /// Get absolute current position, disregarding start and end of window
    pub fn get_raw_pos(&self) -> usize {
        self.pos
    }

    /// Move window start. Ensure new_start <= min(end, pos)
    pub fn set_raw_start(&mut self, s: usize) {
        assert!(s <= self.end, "start must not exceed end");
        assert!(s <= self.pos, "start must not exceed pos");
        self.start = s;
    }

    /// Move window end. Ensure new_end >= max(pos, start) and new_end <= capacity
    pub fn set_raw_end(&mut self, e: usize) {
        let max_bits = self.buffer.len() * 8;
        assert!(e <= max_bits, "end must not exceed capacity");
        assert!(e >= self.start, "end must not be before start");
        assert!(e >= self.pos, "end must not be before pos");
        self.end = e;
    }

    /// Move pos to absolute location, irrespective of window. Ensure start <= pos <= end
    pub fn set_raw_pos(&mut self, p: usize) {
        assert!(self.start <= p, "pos must not be before start");
        assert!(p <= self.end, "pos must not exceed end");
        self.pos = p;
    }

    // String representations /////////////////////////////

    /// Dump bits in window [start, end) as a hex string (uppercase, no separators),
    /// grouping every 4 bits into one hex digit.  If the window length isn't
    /// a multiple of 4, the last nibble is padded on the right with zeros.

    /// --- Dumps only the [start..end) window in hex ---
    pub fn dump_hex(&self) -> String {
        let len = self.end - self.start;
        let n_nibbles = (len + 3) / 4;
        let mut s = String::with_capacity(n_nibbles);
        for i in 0..n_nibbles {
            let bit_pos = self.start + i * 4;
            let bits_left = len - i * 4;
            let take = usize::min(4, bits_left);
            let v = self.read_bits_at_unchecked(bit_pos, take) as u8;
            // if <4 bits, pad low side
            let digit = if take < 4 { v << (4 - take) } else { v };
            s.push_str(&format!("{:X}", digit));
        }
        s
    }

    /// Dump bits in window [start, end) as a binary string of '0'/'1'.
    pub fn dump_bin_unformatted(&self) -> String {
        self.raw_dump_bin(false, false, self.start, self.end)
    }

    /// Dump bits in window [start, end) as a binary string of '0'/'1'.
    /// Adds a ^ marker before the current pos.
    pub fn dump_bin(&self) -> String {
        self.raw_dump_bin(false, true, self.start, self.end)
    }

    /// Dump entire buffer (also outside of window) as a binary string
    /// Optionally adding markers:
    ///  - '<' just before bit `start`
    ///  - '^' just before bit `pos`
    ///  - '>' just before bit `end` (or at end if `end == capacity`)
    /// If trim_to_window is true, contents outside of start/end are not printed.
    pub fn dump_bin_full(&self, print_markers: bool) -> String {
        self.raw_dump_bin(print_markers, print_markers, 0, self.buffer.len() * 8)
    }

    pub fn raw_dump_bin(&self, print_window: bool, print_pos: bool, start: usize, end: usize) -> String {
        let len = end - start;
        let mut s = String::with_capacity(len + if print_window { 2 } else { 0 } + if print_pos { 1 } else { 0 });

        for i in start..end {
            if print_window && i == self.start {
                s.push('<');
            }
            if print_pos && i == self.pos {
                s.push('^');
            }
            if print_window && i == self.end {
                s.push('>');
            }
            let bit = self.read_bits_at_unchecked(i, 1) != 0;
            s.push(if bit { '1' } else { '0' });
        }
        // markers at the very end if needed
        if print_window && self.start == end {
            s.push('<');
        }
        if print_pos && self.pos == end {
            s.push('^');
        }
        if print_window && self.end == end {
            s.push('>');
        }
        s
    }

    /// --- Low‐level unsafe reader: no bounds checks! ---
    /// Reads exactly `num_bits` bits starting at absolute `bit_pos`,
    /// returning them as the low `num_bits` of a `u64`, regardless of window
    /// **Caller must ensure** `num_bits <= 64` and `bit_pos + num_bits <= end`.
    fn read_bits_at_unchecked(&self, mut bit_pos: usize, num_bits: usize) -> u64 {
        // tracing::debug!("read_bits_at_unchecked: {} bits at {}", num_bits, bit_pos);
        // println!("read_bits_at_unchecked: {} bits at {}", num_bits, bit_pos);
        let mut result = 0u64;
        let mut bits_remaining = num_bits;

        // 1) head bits to align to next byte
        let head = bit_pos % 8;
        if head != 0 && bits_remaining > 0 {
            let take = usize::min(8 - head, bits_remaining);
            let byte = self.buffer[bit_pos / 8];
            let shift = 8 - head - take;
            let mask = ((1 << take) - 1) as u8;
            let bits = ((byte >> shift) & mask) as u64;
            result = bits;
            bit_pos += take;
            bits_remaining -= take;
        }

        // 2) full bytes
        while bits_remaining >= 8 {
            let byte = self.buffer[bit_pos / 8] as u64;
            result = (result << 8) | byte;
            bit_pos += 8;
            bits_remaining -= 8;
        }

        // 3) tail bits
        if bits_remaining > 0 {
            let byte = self.buffer[bit_pos / 8];
            for i in 0..bits_remaining {
                let shift = 7 - ((bit_pos % 8) + i);
                let bit = ((byte >> shift) & 1) as u64;
                result = (result << 1) | bit;
            }
        }

        result
    }

    /// --- Low‐level unsafe reader: no bounds checks! ---
    /// Reads 1 bit at absolute `bit_pos`,
    /// **Caller must ensure** `bit_pos + num_bits <= end`.
    fn read_bit_at_unchecked(&self, bit_pos: usize) -> u8 {
        (self.buffer[bit_pos / 8] >> (7 - (bit_pos % 8))) & 1
    }
}

impl fmt::Debug for BitBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BitBuffer {{ <{} ^{} >{} {} }}", self.start, self.pos, self.end, self.dump_bin())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_byte_read_write() {
        let mut bb = BitBuffer::new(16);
        bb.write_bits(0xAB, 8);
        bb.write_bits(0xCD, 8);
        bb.seek(0);
        assert_eq!(bb.read_bits(8).unwrap(), 0xAB);
        assert_eq!(bb.read_bits(8).unwrap(), 0xCD);
    }

    #[test]
    fn test_partial_boundary_read_write() {
        let mut bb = BitBuffer::new(16);
        bb.write_bits(0xA, 4); // 1010
        bb.write_bits(0x5, 4); // 0101
        bb.write_bits(0xFF, 8); // 11111111
        bb.seek(0);
        assert_eq!(bb.read_bits(4).unwrap(), 0xA);
        assert_eq!(bb.read_bits(4).unwrap(), 0x5);
        assert_eq!(bb.read_bits(8).unwrap(), 0xFF);
    }

    #[test]
    fn test_read_overflow() {
        let mut bb = BitBuffer::new(10);
        assert!(bb.read_bits(11).is_none());
        assert_eq!(bb.read_bits(0).unwrap(), 0);
    }

    #[test]
    #[should_panic(expected = "write would exceed buffer end")]
    fn test_write_overflow() {
        let mut bb = BitBuffer::new(10);
        bb.write_bits(1, 11);
    }

    #[test]
    #[should_panic(expected = "value exceeds num_bits")]
    fn test_value_above_num_bits() {
        let mut bb = BitBuffer::new(4);
        bb.write_bits(0b11111, 4);
    }

    #[test]
    fn test_write_autoexpand() {
        let mut bb = BitBuffer::new_autoexpand(10);
        bb.write_bits(1, 5);
        assert_eq!(bb.get_pos(), 5);
        assert_eq!(bb.get_raw_end(), 5);
        bb.write_bits(1, 6);
        assert_eq!(bb.get_pos(), 11);
        assert_eq!(bb.get_raw_end(), 11);
        bb.write_bit(1);
        assert_eq!(bb.get_pos(), 12);
        assert_eq!(bb.get_raw_end(), 12);
    }

    #[test]
    fn test_windowing_and_seek() {
        let mut bb = BitBuffer::from_vec(vec![0xFF, 0x00]);
        bb.set_raw_pos(4);
        bb.set_raw_start(4);
        bb.set_raw_end(12);
        assert_eq!(bb.get_pos(), 0);
        assert_eq!(bb.get_len(), 8);
        bb.seek(0);
        assert_eq!(bb.read_bits(4).unwrap(), 0b1111);
        assert!(bb.read_bits(5).is_none());
    }

    #[test]
    fn test_unaligned_read_write_across_bytes() {
        let mut bb = BitBuffer::new(48);
        bb.seek(5);
        let pattern: u32 = 0b10_1010_1111_0001_0010;
        bb.write_bits(pattern as u64, 20);
        bb.seek(5);
        let got = bb.read_bits(20).unwrap();
        assert_eq!(got as u32, pattern);
    }

    #[test]
    fn test_multi_byte_bulk_read_write() {
        let mut bb = BitBuffer::new(64);
        bb.write_bits(0xDEADBEEF, 32);
        bb.write_bits(0xCAFEBABE, 32);
        bb.seek(0);
        assert_eq!(bb.read_bits(32).unwrap(), 0xDEADBEEF);
        assert_eq!(bb.read_bits(32).unwrap(), 0xCAFEBABE);
    }

    #[test]
    fn test_zero_bit_read_write() {
        let mut bb = BitBuffer::new(16);
        bb.write_bits(0, 0);
        assert_eq!(bb.get_pos(), 0);
        assert_eq!(bb.read_bits(0).unwrap(), 0);
        assert_eq!(bb.get_pos(), 0);
    }

    #[test]
    fn test_read_write_entire_buffer() {
        let mut bb = BitBuffer::new(24);
        bb.write_bits(0xAAAAAA, 24);
        bb.seek(0);
        assert_eq!(bb.read_bits(24).unwrap(), 0xAAAAAA);
        assert_eq!(bb.into_bytes(), vec![0xAA, 0xAA, 0xAA]);
    }

    #[test]
    fn test_dump_hex() {
        let mut bb = BitBuffer::from_vec(vec![0xAB, 0xCD]);
        assert_eq!(bb.dump_hex(), "ABCD");
        bb.set_raw_pos(4);
        bb.set_raw_start(4);
        bb.set_raw_end(12);
        assert_eq!(bb.dump_hex(), "BC");
    }

    #[test]
    fn test_dump_funcs() {
        let mut bb = BitBuffer::from_vec(vec![0xA0]); // 10100000
        // start=0, pos=0, end=8
        assert_eq!(bb.dump_bin_full(true), "<^10100000>");
        // now shift markers:
        bb.seek(3);
        bb.set_raw_start(2);
        bb.set_raw_end(6);
        // buffer=1010 0000; markers at 2:'<', 3:'^', 6:'>'
        assert_eq!(bb.dump_bin_full(true), "10<1^000>00");
        assert_eq!(bb.dump_bin_full(false), "10100000");
        assert_eq!(bb.dump_bin(), "1^000");
        assert_eq!(bb.dump_bin_unformatted(), "1000");
    }

    #[test]
    fn test_peek_bits_basic() {
        // pattern = 0b1011_0110
        let mut bb = BitBuffer::from_bitstr("10110110");
        // peek 4 bits, should be 0b1011, pos unchanged
        let p = bb.peek_bits(4).unwrap();
        assert_eq!(p, 0b1011);
        assert_eq!(bb.get_pos(), 0);

        // now read 4 bits, pos should advance and match peek
        let r = bb.read_bits(4).unwrap();
        assert_eq!(r, p);
        assert_eq!(bb.get_pos(), 4);
    }

    #[test]
    fn test_peek_bits_across_bytes() {
        // 16-bit pattern: 0xABCD = 1010_1011 1100_1101
        let mut bb = BitBuffer::from_bitstr("1010101111001101");
        // advance pos to bit 5
        bb.seek(5);
        assert_eq!(bb.get_pos(), 5);
        // peek 6 bits from [5..11): actually "011110" => 0b011110 == 30
        let p = bb.peek_bits(6).unwrap();
        assert_eq!(p, 0b011110);
        // pos must remain at 5
        assert_eq!(bb.get_pos(), 5);
    }

    #[test]
    fn test_peek_zero_bits() {
        let bb = BitBuffer::new(8);
        // peek zero bits always returns Some(0) and does not advance
        let p = bb.peek_bits(0).unwrap();
        assert_eq!(p, 0);
        assert_eq!(bb.get_pos(), 0);
    }

    #[test]
    fn test_peek_bits_overflow() {
        let bb = BitBuffer::new(10);
        // asking for more than end-start bits should be None
        assert!(bb.peek_bits(11).is_none());
        // pos unchanged
        assert_eq!(bb.get_pos(), 0);

        // even within capacity, peeking exactly to the end is allowed
        assert!(bb.peek_bits(10).is_some());
        assert_eq!(bb.get_pos(), 0);
    }

    #[test]
    fn test_to_bitarr() {
        let mut bb = BitBuffer::from_bitstr("10110011");
        let mut arr = vec![0u8; 8];
        bb.to_bitarr(&mut arr);
        assert_eq!(arr, vec![1, 0, 1, 1, 0, 0, 1, 1]);
    }

    #[test]
    fn test_xor_bit() {
        let mut bb = BitBuffer::from_bitstr("10110000");
        // XOR first bit (1) with 0 -> should remain 1
        bb.xor_bit(0);
        // XOR second bit (0) with 1 -> should become 1
        bb.xor_bit(1);
        // XOR third bit (1) with 1 -> should become 0
        bb.xor_bit(1);
        // bb.seek(0);
        // Check final pattern: original was "10110000", after XORs should be "11010000"
        assert_eq!(bb.to_bitstr(), "11010000");
    }

    #[test]
    fn test_xor_bits() {
        let mut bb = BitBuffer::from_bitstr("0000000000000000");
        let xor_buf = vec![0b11000000, 0b11110000]; // Two bits are set OUTSIDE the xor area
        bb.seek(2);
        bb.xor_bytearr(&xor_buf, 10).unwrap();
        println!("{}", bb.dump_bin());
        assert_eq!(bb.to_bitstr(), "0011000000110000");
    }

    #[test]
    fn test_xor_bits_twochunks() {
        let mut bb = BitBuffer::from_bitstr("000000000000000000000000000000000000000000000000000000000000000000000000");
        let xor_buf = vec![
            0b11000000, 0b11000000, 0b11000000, 0b11000000, 0b11000000, 0b11000000, 0b11000000, 0b11000000, 0b00111100,
        ]; // Two bits are set OUTSIDE the xor area
        bb.seek(2);
        bb.xor_bytearr(&xor_buf, 68).unwrap();
        println!("{}", bb.dump_bin());
        assert_eq!(
            bb.to_bitstr(),
            "001100000011000000110000001100000011000000110000001100000011000000001100"
        );
    }
}
