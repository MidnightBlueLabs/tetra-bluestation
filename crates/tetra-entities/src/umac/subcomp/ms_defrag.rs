use tetra_core::{BitBuffer, TdmaTime, TetraAddress, Todo};

use crate::umac::subcomp::defrag::{DefragBuffer, DefragBufferState};

const DEFRAG_BUF_MAX_LEN: usize = 4096;
const DEFRAG_TS_BEFORE_TIMEOUT: i32 = 10 * 4; // TODO check documentation. 10 frames.

/// Simple defragmenter suitable for MS use
/// Only maintains a single DefragBuffer per timeslot, as only the SwMI will
/// be sending data.
pub struct MsDefrag {
    pub buffers: [DefragBuffer; 4],
}

impl MsDefrag {
    pub fn new() -> Self {
        Self {
            buffers: [DefragBuffer::new(), DefragBuffer::new(), DefragBuffer::new(), DefragBuffer::new()],
        }
    }

    pub fn reset(&mut self) {
        for buffer in &mut self.buffers {
            buffer.reset();
        }
    }

    pub fn age_buffers(&mut self, t: TdmaTime) {
        for buffer in &mut self.buffers {
            if buffer.state != DefragBufferState::Inactive && t.diff(buffer.t_last) > DEFRAG_TS_BEFORE_TIMEOUT {
                tracing::warn!("Defrag buffer {} timed out", buffer.t_last.t);
                buffer.reset();
            }
        }
    }

    /// Inserts a first fragment into a fragbuffer.
    pub fn insert_first(&mut self, bitbuffer: &mut BitBuffer, t: TdmaTime, addr: TetraAddress, aie_info: Option<Todo>) {
        // Reset target buffer if needed
        let ts = (t.t - 1) as usize;
        if self.buffers[ts].state != DefragBufferState::Inactive {
            tracing::warn!("Defrag buffer {} not inactive (state: {:?})", ts, self.buffers[ts].state);
            self.buffers[ts].reset();
        }

        // Initialize target buffer
        self.buffers[ts].state = DefragBufferState::Active;
        self.buffers[ts].addr = addr;
        self.buffers[ts].t_first = t;
        self.buffers[ts].t_last = t;
        self.buffers[ts].num_frags = 1;
        self.buffers[ts].aie_info = aie_info;

        // Copy the bitbuffer data from pos to end into our fragbuffer
        self.buffers[ts].buffer.copy_bits(bitbuffer, bitbuffer.get_len_remaining());

        tracing::debug!(
            "Defrag buffer {} first: ssi: {}, t_first: {}, t_last: {}, num_frags: {}: {}",
            ts,
            self.buffers[ts].addr.ssi,
            self.buffers[ts].t_first,
            self.buffers[ts].t_last,
            self.buffers[ts].num_frags,
            self.buffers[ts].buffer.dump_bin()
        );
    }

    pub fn insert_next(&mut self, bitbuffer: &mut BitBuffer, t: TdmaTime) {
        let ts = (t.t - 1) as usize;
        if self.buffers[ts].state != DefragBufferState::Active {
            tracing::warn!("Defrag buffer {} is not active", ts);
            return;
        }

        if self.buffers[ts].buffer.get_len() + bitbuffer.get_len_remaining() > DEFRAG_BUF_MAX_LEN {
            tracing::warn!("Defrag buffer {} would exceed max len", ts);
            self.buffers[ts] = DefragBuffer::new();
            return;
        }

        self.buffers[ts].t_last = t;
        self.buffers[ts].num_frags += 1;

        // Copy the bitbuffer data from pos to end into our fragbuffer
        self.buffers[ts].buffer.copy_bits(bitbuffer, bitbuffer.get_len_remaining());

        tracing::debug!(
            "Defrag buffer {} next:  ssi: {}, t_first: {}, t_last: {}, num_frags: {}: {}",
            ts,
            self.buffers[ts].addr.ssi,
            self.buffers[ts].t_first,
            self.buffers[ts].t_last,
            self.buffers[ts].num_frags,
            self.buffers[ts].buffer.dump_bin()
        );
    }

    pub fn insert_last(&mut self, bitbuffer: &mut BitBuffer, t: TdmaTime) {
        let ts = (t.t - 1) as usize;
        if self.buffers[ts].state != DefragBufferState::Active {
            tracing::warn!("Defrag buffer {} is not active", ts);
            return;
        }

        if self.buffers[ts].buffer.get_len() + bitbuffer.get_len_remaining() > DEFRAG_BUF_MAX_LEN {
            tracing::warn!("Defrag buffer {} would exceed max len", ts);
            self.buffers[ts] = DefragBuffer::new();
            return;
        }

        self.buffers[ts].state = DefragBufferState::Complete;
        self.buffers[ts].t_last = t;
        self.buffers[ts].num_frags += 1;

        // Copy the bitbuffer data from pos to end into our fragbuffer
        self.buffers[ts].buffer.copy_bits(bitbuffer, bitbuffer.get_len_remaining());

        tracing::debug!(
            "Defrag buffer {} last:  ssi: {}, t_first: {}, t_last: {}, num_frags: {}: {}",
            ts,
            self.buffers[ts].addr.ssi,
            self.buffers[ts].t_first,
            self.buffers[ts].t_last,
            self.buffers[ts].num_frags,
            self.buffers[ts].buffer.dump_bin()
        );
    }

    /// Retrieves a reference to the AIE info associated with a defrag buffer
    pub fn get_aie_info(&self, t: TdmaTime) -> Option<&Todo> {
        let ts = (t.t - 1) as usize;
        if self.buffers[ts].state != DefragBufferState::Active {
            tracing::warn!("Defrag buffer {} is not active", ts);
            return None;
        }
        self.buffers[ts].aie_info.as_ref()
    }

    /// Transfers finalized defragbuf to caller, setting bitbuffer slot pos to start.
    pub fn take_defragged_buf(&mut self, t: TdmaTime) -> Option<DefragBuffer> {
        let ts = (t.t - 1) as usize;
        if self.buffers[ts].state != DefragBufferState::Complete {
            tracing::warn!("Defrag buffer {} is not complete", ts);
            return None;
        }

        // Take the slot out of the fragbuffer and return it
        // We also re-initialize the fragbuffer slot with a fresh one
        let mut defragbuffer = std::mem::replace(&mut self.buffers[ts], DefragBuffer::new());
        defragbuffer.buffer.set_raw_end(defragbuffer.buffer.get_raw_pos());
        defragbuffer.buffer.seek(0);

        Some(defragbuffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tetra_core::{address::SsiType, bitbuffer::BitBuffer, debug};

    #[test]
    fn test_3_chunks() {
        debug::setup_logging_verbose();

        let mut buf1 = BitBuffer::from_bitstr("000");
        let t1 = TdmaTime::default().add_timeslots(2); // UL time 0
        let mut buf2 = BitBuffer::from_bitstr("111");
        let t2 = t1.add_timeslots(4);
        let mut buf3 = BitBuffer::from_bitstr("0011");
        let t3 = t2.add_timeslots(4);

        let mut defragger = MsDefrag::new();
        defragger.insert_first(
            &mut buf1,
            t1,
            TetraAddress {
                ssi: 1234,
                ssi_type: SsiType::Issi,
                encrypted: false,
            },
            None,
        );
        defragger.insert_next(&mut buf2, t2);
        defragger.insert_last(&mut buf3, t3);

        let out = defragger.take_defragged_buf(t3).unwrap();
        assert_eq!(out.buffer.to_bitstr(), "0001110011");
    }
}
