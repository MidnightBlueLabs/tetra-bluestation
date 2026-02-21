use std::cmp::min;

use tetra_core::BitBuffer;

use tetra_pdus::umac::pdus::{mac_end_dl::MacEndDl, mac_frag_dl::MacFragDl, mac_resource::MacResource};

use crate::umac::subcomp::fillbits;

#[derive(Debug)]
pub struct BsFragger {
    resource: MacResource,
    mac_hdr_is_written: bool,
    done: bool,
    sdu: BitBuffer,
}

/// We won't start fragmentation if less than MIN_SLOT_CAP_FOR_FRAG_START bits are free in the slot
const MIN_SLOT_CAP_FOR_RES_FRAG_START: usize = 32;

/// We won't insert a fragment if less than MIN_SLOT_CAP_FOR_FRAG bits are free in the slot
const MIN_SLOT_CAP_FOR_FRAG: usize = 16;

impl BsFragger {
    pub fn new(resource: MacResource, sdu: BitBuffer) -> Self {
        assert!(sdu.get_pos() == 0, "SDU must be at the start of the buffer");
        // We set the length field now. If we do fragmentation, we'll set it to -1 later.
        // resource.update_len_and_fill_ind(sdu.get_len());
        BsFragger {
            resource,
            mac_hdr_is_written: false,
            done: false,
            sdu,
        }
    }

    /// True if this fragger targets a group address (GSSI). Used to avoid starving MM responses on TS1.
    pub fn is_group_addr(&self) -> bool {
        matches!(
            self.resource.addr,
            Some(a) if a.ssi_type == tetra_core::address::SsiType::Gssi
        )
    }

    /// True if the MAC-RESOURCE header has already been emitted (i.e. fragmentation is in progress).
    pub fn has_started(&self) -> bool {
        self.mac_hdr_is_written
    }

    /// True if the MAC-RESOURCE carries a ChanAlloc element.
    pub fn has_chan_alloc(&self) -> bool {
        self.resource.chan_alloc_element.is_some()
    }

    /// Writes MAC-RESOURCE to dest_buf, starting fragmentation if needed.
    /// Then, writes as many SDU bits as possible.
    /// Returns true if the entire SDU was consumed, false if the PDU is fragmented
    /// and more chunks are needed.
    fn get_resource_chunk(&mut self, mac_block: &mut BitBuffer) -> bool {
        // Some sanity checks
        assert!(self.sdu.get_pos() == 0, "SDU must be at the start of the buffer");
        assert!(!self.mac_hdr_is_written, "MAC header should not be written yet");
        assert!(
            !(self.resource.is_null_pdu() && self.sdu.get_len_remaining() > 0),
            "Null PDU cannot have SDU data"
        );

        // Compute len of full resource, including sdu and fill bits
        let hdr_len_bits = self.resource.compute_header_len();
        let sdu_len_bits = self.sdu.get_len_remaining();
        let slot_cap_bits = mac_block.get_len_remaining();

        let num_fill_bits = fillbits::addition::compute_required(hdr_len_bits + sdu_len_bits, slot_cap_bits);

        let total_len_bits = hdr_len_bits + sdu_len_bits + num_fill_bits;
        let total_len_bytes = total_len_bits / 8;

        // Check if we can fit all in a single MAC-RESOURCE
        if total_len_bits <= slot_cap_bits {
            // Fits in one MAC-RESOURCE
            assert!(
                total_len_bits % 8 == 0 || total_len_bits == mac_block.get_len_remaining(),
                "PDU must fill slot or have byte aligned end, got len {} for remaining cap {}",
                total_len_bits,
                mac_block.get_len_remaining()
            );

            // Update PDU fields
            self.resource.length_ind = total_len_bytes as u8;
            self.resource.fill_bits = num_fill_bits > 0;

            tracing::debug!(
                "-> {:?} sdu {}",
                self.resource,
                self.sdu
                    .raw_dump_bin(false, false, self.sdu.get_pos(), self.sdu.get_pos() + sdu_len_bits)
            );

            // Write MAC-RESOURCE header, followed by TM-SDU, to MAC block
            self.resource.to_bitbuf(mac_block);
            mac_block.copy_bits(&mut self.sdu, sdu_len_bits);
            fillbits::addition::write(mac_block, Some(num_fill_bits));

            // We're done with this packet
            self.mac_hdr_is_written = true;
            true
        } else if slot_cap_bits < MIN_SLOT_CAP_FOR_RES_FRAG_START || slot_cap_bits < hdr_len_bits {
            // Not enough room to start fragmentation: either the remaining slot capacity is
            // below the minimum threshold, or the MAC-RESOURCE header alone doesn't fit in the
            // remaining space. Defer the entire PDU to the next frame.
            tracing::debug!(
                "-> does_not_fit (cap={} hdr={}), trying again next frame",
                slot_cap_bits,
                hdr_len_bits
            );
            false
        } else {
            // We need to start fragmentation. No fill bits are needed
            // Avoid starting fragmentation with a tiny first fragment. Some terminals are sensitive to
            // interleaved / micro-fragmented MAC-RESOURCE sequences (especially group-call signalling with ChanAlloc).
            let sdu_bits_avail = slot_cap_bits - hdr_len_bits;
            let min_first_frag_bits = if self.is_group_addr() || self.resource.chan_alloc_element.is_some() { MIN_SLOT_CAP_FOR_FRAG } else { 0 };
            if sdu_bits_avail < min_first_frag_bits {
                tracing::debug!(
                    "-> frag_start_too_small (cap={} hdr={} sdu_bits_avail={} min={}), trying again next frame",
                    slot_cap_bits, hdr_len_bits, sdu_bits_avail, min_first_frag_bits
                );
                return false;
            }
            self.resource.length_ind = 0b111111; // Start of fragmentation
            self.resource.fill_bits = false;
            assert!(num_fill_bits == 0, "Got {} fill bits upon frag start", num_fill_bits);
            let sdu_bits = slot_cap_bits - hdr_len_bits;

            tracing::debug!(
                "-> {:?} sdu {}",
                self.resource,
                self.sdu
                    .raw_dump_bin(false, false, self.sdu.get_pos(), self.sdu.get_pos() + sdu_bits)
            );

            self.resource.to_bitbuf(mac_block);
            mac_block.copy_bits(&mut self.sdu, sdu_bits);
            fillbits::addition::write(mac_block, None);

            // More fragments follow
            self.mac_hdr_is_written = true;
            false
        }
    }

    /// After MAC-RESOURCE was output using get_first_chunk, call this function to consume
    /// next chunks. Based on capacity, will determine whether to make a MAC-FRAG or
    /// MAC-END.
    /// Returns true when MAC-END (DL) was created and no further fragments are needed
    /// TODO FIXME: support adding ChanAlloc element in MAC-END
    fn get_frag_or_end_chunk(&mut self, mac_block: &mut BitBuffer) -> bool {
        // Some sanity checks
        assert!(self.mac_hdr_is_written, "MAC header should be previously written");

        // Check if we can fit all in a MAC-END message
        let sdu_bits = self.sdu.get_len_remaining();
        let macend_len_bits = MacEndDl::compute_hdr_len(false, false) + sdu_bits;
        let macend_len_bytes = (macend_len_bits + 7) / 8;
        let slot_cap_bits = mac_block.get_len_remaining();

        // tracing::trace!("MAC-END would have length: {} bits, {} bytes, slot capacity: {} bits",
        //     macend_len_bits, macend_len_bytes, slot_cap);
        if macend_len_bytes * 8 <= slot_cap_bits {
            // Fits in single MAC-END
            let num_fill_bits = fillbits::addition::compute_required(macend_len_bits, slot_cap_bits);
            let pdu = MacEndDl {
                fill_bits: num_fill_bits > 0,
                pos_of_grant: 0,
                length_ind: macend_len_bytes as u8,
                slot_granting_element: None,
                chan_alloc_element: None,
            };

            tracing::debug!(
                "-> {:?} sdu {}",
                pdu,
                self.sdu
                    .raw_dump_bin(false, false, self.sdu.get_pos(), self.sdu.get_pos() + sdu_bits)
            );

            // Write MAC-END header followed by TM-SDU
            pdu.to_bitbuf(mac_block);
            mac_block.copy_bits(&mut self.sdu, sdu_bits);

            // Write fill bits (if needed)
            if num_fill_bits > 0 {
                mac_block.write_bit(1);
                mac_block.write_zeroes(num_fill_bits - 1);
            }
            // We're done with this packet
            true
        } else if slot_cap_bits < MIN_SLOT_CAP_FOR_FRAG {
            // Not worth (or possible) to place a fragment here. Rather wait for a new slot
            // We do nothing and simply return that more work is needed
            tracing::debug!("-> does_not_fit, trying again next frame");
            false
        } else {
            // Need MAC-FRAG, fill slot (or don't fill, if the MAC-END hdr size is the reason we go for MAC-FRAG)
            let macfrag_hdr_len = 4;
            let sdu_bits_in_frag = min(slot_cap_bits - macfrag_hdr_len, sdu_bits);
            let num_fill_bits = slot_cap_bits - macfrag_hdr_len - sdu_bits_in_frag;

            let pdu = MacFragDl {
                fill_bits: num_fill_bits > 0,
            };

            tracing::debug!(
                "-> {:?} sdu {}",
                pdu,
                self.sdu
                    .raw_dump_bin(false, false, self.sdu.get_pos(), self.sdu.get_pos() + sdu_bits)
            );

            pdu.to_bitbuf(mac_block);
            mac_block.copy_bits(&mut self.sdu, sdu_bits_in_frag);

            if num_fill_bits > 0 {
                mac_block.write_bit(1);
                mac_block.write_zeroes(num_fill_bits - 1);
            }

            false
        }
    }

    /// Writes the next chunk to the bitbuffer, if there is space.
    /// First chunk is the provided resource, possibly changed to indicate fragmentation.
    /// Subsequent chunks are MAC-FRAG or MAC-END.
    /// Returns (bool is_done, usize bits_written)
    pub fn get_next_chunk(&mut self, mac_block: &mut BitBuffer) -> bool {
        assert!(!self.done, "all fragments have already been produced");
        assert!(
            mac_block.get_len_written() % 8 == 0 || mac_block.get_len_remaining() == 0,
            "mac_block must be full or byte aligned before writing"
        );

        self.done = if !self.mac_hdr_is_written {
            // First chunk, write MAC-RESOURCE
            self.get_resource_chunk(mac_block)
        } else {
            // Subsequent chunks, write MAC-FRAG or MAC-END
            self.get_frag_or_end_chunk(mac_block)
        };

        self.done
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::{
        address::{SsiType, TetraAddress},
        debug,
    };

    use crate::umac::subcomp::bs_sched::{SCH_F_CAP, SCH_HD_CAP};

    use super::*;
    fn get_default_resource() -> MacResource {
        MacResource {
            fill_bits: false,
            pos_of_grant: 0,
            encryption_mode: 0,
            random_access_flag: false,
            length_ind: 0,
            addr: Some(TetraAddress {
                encrypted: false,
                ssi_type: SsiType::Ssi,
                ssi: 1234,
            }),
            event_label: None,
            usage_marker: None,
            power_control_element: None,
            slot_granting_element: None,
            chan_alloc_element: None,
        }
    }

    #[test]
    fn test_single_chunk() {
        debug::setup_logging_verbose();
        let pdu = get_default_resource();
        let sdu = BitBuffer::from_bitstr("111000111");
        let mut mac_block = BitBuffer::new(SCH_F_CAP);

        let mut fragger = BsFragger::new(pdu, sdu);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);

        assert!(done, "Should be done in single chunk");
        tracing::info!("MAC block: {}", mac_block.dump_bin());
    }

    #[test]
    fn test_four_chunks() {
        debug::setup_logging_verbose();
        let vec = "01010110010011000010101010010010110101010110010011001011111110101011001010010110111001011111111111100010011000000011010011001110010111110010100100010111010110000010010001101000011000000111101011010001001111001110110100000101010111110100010000100101001100011110010111001010101001110110111010001001101101111100111001000001111100101010000010111";
        let mut reconstructed = String::new();
        let pdu = get_default_resource();
        let sdu = BitBuffer::from_bitstr(vec);
        let mut fragger = BsFragger::new(pdu, sdu);

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacResource::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[1]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        // tracing::info!("[1] reconstructed so far: {}", reconstructed);
        assert!(!done, "Should take four blocks");

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacFragDl::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[2]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        // tracing::info!("[1] reconstructed so far: {}", reconstructed);
        assert!(!done, "Should take four blocks");

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacFragDl::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[3]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        // tracing::info!("[1] reconstructed so far: {}", reconstructed);
        assert!(!done, "Should take four blocks");

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacEndDl::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[4]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        tracing::info!("     Reconstructed: {}", reconstructed);
        assert!(done, "Should take four blocks");

        // Test that the original vec is contained in the reconstructed string
        // We'll just assume the fill bits check out..
        assert!(
            reconstructed.starts_with(vec),
            "Original vec should be contained in reconstructed string"
        );
    }

    #[test]
    fn test_low_cap_start_and_no_room_for_fill_bits() {
        // TODO FIXME: after further reading of the spec, while the searched behavior is not incorrect,
        // this test is suboptimal. The SDU entirely fits into the second mac_block, but fill bits would
        // not fit. HOwever, the spec states we may supply a higher lenght_ind, and the effective lenght
        // will be capped by the mac_block size. Thus, no fill bits need to be added.
        // TODO: adapt the behavior, and adapt test to verify sdu fits exactly into the second slot.

        debug::setup_logging_verbose();
        let vec = "010101100100110000";
        let pdu = get_default_resource();
        let sdu = BitBuffer::from_bitstr(vec);
        let mut fragger = BsFragger::new(pdu, sdu);

        let mut mac_block = BitBuffer::new(30); // Too small for proper message
        let done = fragger.get_next_chunk(&mut mac_block);
        tracing::info!("[1]: {}", mac_block.dump_bin());
        assert!(!done);

        let mut mac_block = BitBuffer::new(61); // Contains all SDU bits; but can't fit fill bits??
        let done = fragger.get_next_chunk(&mut mac_block);
        tracing::info!("[2]: {}", mac_block.dump_bin());
        assert!(!done, "fill bits shouldnt fit");

        let mut mac_block = BitBuffer::new(61); // Too small for proper message
        let done = fragger.get_next_chunk(&mut mac_block);
        tracing::info!("[3]: {}", mac_block.dump_bin());
        assert!(done);
    }
}
