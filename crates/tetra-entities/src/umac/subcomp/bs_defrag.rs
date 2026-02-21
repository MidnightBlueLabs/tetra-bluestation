use std::collections::HashMap;

use tetra_core::{BitBuffer, TdmaTime, TetraAddress, Todo};

use crate::umac::subcomp::defrag::{DefragBuffer, DefragBufferState};

const DEFRAG_BUF_MAX_LEN: usize = 4096;
const DEFRAG_TS_BEFORE_TIMEOUT: i32 = 10 * 4; // TODO check documentation. 10 frames.

/// Defragmenter suitable for BS use
/// Maintains a set of DefragBuffers per timeslot, indexed by SSI.
/// This allows multiple MSes to send fragmented data in the same timeslot.
pub struct BsDefrag {
    pub buffers: [HashMap<u32, DefragBuffer>; 4],
}

impl BsDefrag {
    pub fn new() -> Self {
        Self {
            buffers: [HashMap::new(), HashMap::new(), HashMap::new(), HashMap::new()],
        }
    }

    pub fn reset(&mut self) {
        for map in &mut self.buffers {
            map.clear();
        }
    }

    pub fn age_buffers(&mut self, t: TdmaTime) {
        for map in &mut self.buffers {
            for buffer in map.values_mut() {
                if buffer.state != DefragBufferState::Inactive && t.diff(buffer.t_last) > DEFRAG_TS_BEFORE_TIMEOUT {
                    tracing::info!("defrag_buffer for {} timed out", buffer.t_last.t);
                    buffer.reset();
                }
            }
        }
    }

    /// Inserts a first fragment into a fragbuffer.
    pub fn insert_first(&mut self, bitbuffer: &mut BitBuffer, t: TdmaTime, addr: TetraAddress, aie_info: Option<Todo>) {
        // Check if buffer already exists for this ssi/timeslot
        // Remove and discard, if so.
        let ts = (t.t - 1) as usize;
        let ssi = addr.ssi;
        let mut buf = if let Some(mut buf) = self.buffers[ts].remove(&ssi) {
            tracing::warn!("defrag_buffer for ts {} ssi {} in not inactive (state: {:?})", t.t, ssi, buf.state);
            buf.reset();
            buf
        } else {
            DefragBuffer::new()
        };

        // Initialize target buffer
        buf.state = DefragBufferState::Active;
        buf.addr = addr;
        buf.t_first = t;
        buf.t_last = t;
        buf.num_frags = 1;
        buf.aie_info = aie_info;

        // Copy the bitbuffer data from pos to end into our fragbuffer
        buf.buffer.copy_bits(bitbuffer, bitbuffer.get_len_remaining());

        tracing::debug!(
            "defrag_buffer for ts {} ssi: {}, t: {}-{}, frags: {}: {}",
            ts,
            buf.addr.ssi,
            buf.t_first,
            buf.t_last,
            buf.num_frags,
            buf.buffer.dump_bin()
        );

        self.buffers[ts].insert(ssi, buf);
    }

    /// Discard any active defragmentation buffer for the given SSI in the given timeslot.
    /// Used for strict ETSI handling of stolen second half-slot fragmentation (23.8.4.1.4).
    pub fn discard(&mut self, ssi: u32, t: TdmaTime) {
        let ts = (t.t - 1) as usize;
        if let Some(mut buf) = self.buffers[ts].remove(&ssi) {
            tracing::warn!("defrag_buffer discard ts {} ssi {} (state: {:?}, frags: {})", t.t, ssi, buf.state, buf.num_frags);
            buf.reset();
        }
    }

    pub fn insert_next(&mut self, bitbuffer: &mut BitBuffer, ssi: u32, t: TdmaTime) {
        let ts = (t.t - 1) as usize;
        let buf = match self.buffers[ts].get_mut(&ssi) {
            Some(b) => b,
            None => {
                tracing::warn!("defrag_buffer for ts {} ssi {} not found", t.t, ssi);
                return;
            }
        };

        if buf.state != DefragBufferState::Active {
            tracing::warn!("defrag_buffer for ts {} ssi {} not active", t.t, ssi);
            return;
        }

        if buf.buffer.get_len() + bitbuffer.get_len_remaining() > DEFRAG_BUF_MAX_LEN {
            tracing::warn!("defrag_buffer for ts {} ssi {} would exceed max len", t.t, ssi);
            buf.reset();
            return;
        }

        buf.t_last = t;
        buf.num_frags += 1;

        // Copy the bitbuffer data from pos to end into our fragbuffer
        buf.buffer.copy_bits(bitbuffer, bitbuffer.get_len_remaining());

        tracing::debug!(
            "defrag_buffer for ts {} ssi: {}, t: {}-{}, frags: {}: {}",
            t.t,
            ssi,
            buf.t_first,
            buf.t_last,
            buf.num_frags,
            buf.buffer.dump_bin()
        );
    }

    /// Inserts the last fragment into a DefragBuffer, and returns the completed object
    pub fn insert_last(&mut self, bitbuffer: &mut BitBuffer, ssi: u32, t: TdmaTime) -> Option<DefragBuffer> {
        // First, insert the last fragment, then reset buffer pos to start
        self.insert_next(bitbuffer, ssi, t);

        // Now take the buffer out of the map
        let ts = (t.t - 1) as usize;
        let mut buf = match self.buffers[ts].remove(&ssi) {
            Some(b) => b,
            None => {
                tracing::warn!("defrag_buffer for ts {} ssi {} not found", t.t, ssi);
                return None;
            }
        };

        // Update state to complete and return
        buf.state = DefragBufferState::Complete;
        buf.buffer.set_raw_pos(0);
        Some(buf)
    }

    /// Retrieves a read-only reference to the AIE info associated with a DefragBuffer
    pub fn get_aie_info(&self, ssi: u32, t: TdmaTime) -> Option<&Todo> {
        let ts = (t.t - 1) as usize;
        let buf = match self.buffers[ts].get(&ssi) {
            Some(b) => b,
            None => {
                tracing::warn!("defrag_buffer for ts {} ssi {} not found", t.t, ssi);
                return None;
            }
        };
        if buf.state == DefragBufferState::Inactive {
            tracing::warn!("defrag_buffer for ts {} ssi {} not active", t.t, ssi);
            return None;
        };
        buf.aie_info.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tetra_core::{address::SsiType, bitbuffer::BitBuffer, debug};

    #[test]
    fn test_3_chunks() {
        debug::setup_logging_verbose();

        let ssi = 1234;
        let mut buf1 = BitBuffer::from_bitstr("000");
        let t1 = TdmaTime::default().add_timeslots(2); // UL time 0
        let mut buf2 = BitBuffer::from_bitstr("111");
        let t2 = t1.add_timeslots(4);
        let mut buf3 = BitBuffer::from_bitstr("0011");
        let t3 = t2.add_timeslots(4);

        let mut defragger = BsDefrag::new();
        let addr = TetraAddress {
            ssi,
            ssi_type: SsiType::Issi,
            encrypted: false,
        };
        defragger.insert_first(&mut buf1, t1, addr, None);
        defragger.insert_next(&mut buf2, ssi, t2);
        let out = defragger.insert_last(&mut buf3, ssi, t3).unwrap();
        assert_eq!(out.buffer.to_bitstr(), "0001110011");
        assert_eq!(out.buffer.get_pos(), 0);
    }
}
