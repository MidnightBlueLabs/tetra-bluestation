use std::cmp::min;

use crate::umac::subcomp::fillbits;
use tetra_core::{BitBuffer, TxReporter};
use tetra_pdus::umac::fields::channel_allocation::ChanAllocElement;
use tetra_pdus::umac::pdus::mac_resource::MAC_RESOURCE_LENGTH_FRAG_START;
use tetra_pdus::umac::pdus::{mac_end_dl::MacEndDl, mac_frag_dl::MacFragDl, mac_resource::MacResource};

#[derive(Debug)]
pub struct BsFragger {
    resource: MacResource,
    chan_alloc: Option<ChanAllocElement>,
    mac_hdr_is_written: bool,
    is_fully_transmitted: bool,
    sdu: BitBuffer,
    tx_reporter: Option<TxReporter>,
}

/// We won't start fragmentation if less than MIN_SLOT_CAP_FOR_FRAG_START bits are free in the slot
const MIN_SLOT_CAP_FOR_RES_FRAG_START: usize = 32;

/// We won't insert a fragment if less than MIN_SLOT_CAP_FOR_FRAG bits are free in the slot
const MIN_SLOT_CAP_FOR_FRAG: usize = 16;

impl BsFragger {
    pub fn new(resource: MacResource, sdu: BitBuffer, tx_reporter: Option<TxReporter>) -> Self {
        assert!(sdu.get_pos() == 0, "SDU must be at the start of the buffer");
        // We set the length field now. If we do fragmentation, we'll set it to -1 later.
        // resource.update_len_and_fill_ind(sdu.get_len());
        BsFragger {
            resource,
            chan_alloc: None,
            mac_hdr_is_written: false,
            is_fully_transmitted: false,
            sdu,
            tx_reporter,
        }
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
        let mut hdr_len_bits = self.resource.compute_header_len();
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
            let mut sdu_space_bits = slot_cap_bits - hdr_len_bits;

            // If there is a channel allocation element, this needs to be delayed until the last fragment.
            // 23.5.4.1 - "The channel allocation is generally sent in a MAC-RESOURCE PDU. However, if the BS wishes to send channel
            // allocation information with a fragmented message then that information shall be included within the MAC-END PDU
            // and shall not be included within the MAC-RESOURCE PDU."
            if self.resource.chan_alloc_element.is_some() {

                // Move the chan_alloc element out of the resource and into the fragger state
                self.chan_alloc = self.resource.chan_alloc_element.clone();
                self.resource.chan_alloc_element = None;

                // Compute the new header length & the new available space for SDU bits
                hdr_len_bits = self.resource.compute_header_len();
                sdu_space_bits = slot_cap_bits - hdr_len_bits;

                // If the SDU would now fit, do not start fragmentation and instead defer the PDU
                if sdu_len_bits <= sdu_space_bits {
                    tracing::debug!("PDU fits after removing channel allocation, shan't fragment - deferring instead");

                    // Put the channel allocation element back into the resource
                    self.resource.chan_alloc_element = self.chan_alloc.take();

                    return false;
                }
            }

            // We need to start fragmentation
            self.resource.length_ind = MAC_RESOURCE_LENGTH_FRAG_START;

            // No fill bits are allowed.
            self.resource.fill_bits = false;

            tracing::debug!(
                "-> Fragged {:?} sdu {}",
                self.resource,
                self.sdu
                    .raw_dump_bin(false, false, self.sdu.get_pos(), self.sdu.get_pos() + sdu_space_bits)
            );

            self.resource.to_bitbuf(mac_block);
            mac_block.copy_bits(&mut self.sdu, sdu_space_bits);
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
    fn get_frag_or_end_chunk(&mut self, mac_block: &mut BitBuffer) -> bool {
        // Some sanity checks
        assert!(self.mac_hdr_is_written, "MAC header should be previously written");

        // Check if we can fit all in a MAC-END message
        let sdu_bits = self.sdu.get_len_remaining();
        let macend_len_bits = MacEndDl::compute_hdr_len(None, self.chan_alloc.clone()) + sdu_bits;
        let macend_len_bytes = (macend_len_bits + 7) / 8;
        let slot_cap_bits = mac_block.get_len_remaining();

        // tracing::trace!("MAC-END would have length: {} bits, {} bytes, slot capacity: {} bits",
        //     macend_len_bits, macend_len_bytes, slot_cap);
        if macend_len_bytes * 8 <= slot_cap_bits {
            // Fits in single MAC-END
            let num_fill_bits = fillbits::addition::compute_required(macend_len_bits, slot_cap_bits);
            let mut pdu = MacEndDl {
                fill_bits: num_fill_bits > 0,
                pos_of_grant: 0,
                length_ind: macend_len_bytes as u8,
                slot_granting_element: None,
                chan_alloc_element: None,
            };

            if let Some(chan_alloc) = self.chan_alloc.take() {
                tracing::debug!("Placing deferred channel allocation element in MAC-END");
                pdu.chan_alloc_element = Some(chan_alloc);
            }

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
    /// Returns bool is_fully_transmitted
    pub fn get_next_chunk(&mut self, mac_block: &mut BitBuffer) -> bool {
        assert!(!self.is_fully_transmitted, "all fragments have already been produced");
        assert!(
            mac_block.get_len_written() % 8 == 0 || mac_block.get_len_remaining() == 0,
            "mac_block must be full or byte aligned before writing"
        );

        self.is_fully_transmitted = if !self.mac_hdr_is_written {
            // First chunk, write MAC-RESOURCE
            self.get_resource_chunk(mac_block)
        } else {
            // Subsequent chunks, write MAC-FRAG or MAC-END
            self.get_frag_or_end_chunk(mac_block)
        };

        // If we're done now, we'll report the PDUs full transmission.
        if self.is_fully_transmitted
            && let Some(tx_reporter) = &self.tx_reporter
        {
            tx_reporter.mark_transmitted();
        }

        self.is_fully_transmitted
    }
}

impl Drop for BsFragger {
    fn drop(&mut self) {
        if !self.is_fully_transmitted
            && let Some(tx_reporter) = &self.tx_reporter
            && tx_reporter.get_state() == tetra_core::TxState::Pending
        {
            tx_reporter.mark_discarded();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::umac::subcomp::bs_sched::{SCH_F_CAP, SCH_HD_CAP};
    use tetra_core::{
        TxState,
        address::{SsiType, TetraAddress},
        debug,
    };
    use tetra_pdus::umac::pdus::mac_resource::MAC_RESOURCE_LENGTH_FRAG_START;
    use tetra_saps::lcmc::enums::alloc_type::ChanAllocType;
    use tetra_saps::lcmc::enums::ul_dl_assignment::UlDlAssignment;

    use super::*;
    fn get_default_resource() -> MacResource {
        MacResource {
            fill_bits: false,
            pos_of_grant: 0,
            encryption_mode: 0,
            random_access_flag: false,
            length_ind: 0,
            addr: Some(TetraAddress {
                ssi_type: SsiType::Issi,
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

        let mut fragger = BsFragger::new(pdu, sdu, None);
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
        let mut fragger = BsFragger::new(pdu, sdu, None);

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
    fn test_four_chunks_with_tx_reporter() {
        debug::setup_logging_verbose();
        let vec = "01010110010011000010101010010010110101010110010011001011111110101011001010010110111001011111111111100010011000000011010011001110010111110010100100010111010110000010010001101000011000000111101011010001001111001110110100000101010111110100010000100101001100011110010111001010101001110110111010001001101101111100111001000001111100101010000010111";
        let mut reconstructed = String::new();
        let pdu = get_default_resource();
        let sdu = BitBuffer::from_bitstr(vec);
        let reporter = TxReporter::new_unacked();
        let mut fragger = BsFragger::new(pdu, sdu, Some(reporter.clone()));

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacResource::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[1]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        // tracing::info!("[1] reconstructed so far: {}", reconstructed);
        assert!(!done, "Should take four blocks");
        assert!(!reporter.is_in_final_state() && !reporter.is_transmitted());

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacFragDl::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[2]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        // tracing::info!("[1] reconstructed so far: {}", reconstructed);
        assert!(!done, "Should take four blocks");
        assert!(!reporter.is_in_final_state() && !reporter.is_transmitted());

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacFragDl::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[3]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        // tracing::info!("[1] reconstructed so far: {}", reconstructed);
        assert!(!done, "Should take four blocks");
        assert!(!reporter.is_in_final_state() && !reporter.is_transmitted());

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);
        let pdu = MacEndDl::from_bitbuf(&mut mac_block).unwrap();
        mac_block.set_raw_start(mac_block.get_raw_pos());
        tracing::info!("[4]: {}: {}", pdu, mac_block.dump_bin());
        reconstructed += &mac_block.to_bitstr();
        tracing::info!("     Reconstructed: {}", reconstructed);
        assert!(done, "Should take four blocks");
        assert!(reporter.is_in_final_state() && reporter.is_transmitted());

        // Test that the original vec is contained in the reconstructed string
        // We'll just assume the fill bits check out..
        assert!(
            reconstructed.starts_with(vec),
            "Original vec should be contained in reconstructed string"
        );
    }

    #[test]
    fn test_drop_marks_discarded_when_not_fully_transmitted() {
        debug::setup_logging_verbose();
        let pdu = get_default_resource();
        let sdu = BitBuffer::from_bitstr("10101010");
        let reporter = TxReporter::new_unacked();

        let _fragger = BsFragger::new(pdu, sdu, Some(reporter.clone()));
        drop(_fragger);

        assert_eq!(reporter.get_state(), TxState::Discarded);
        assert!(reporter.is_in_final_state());
        assert!(!reporter.is_transmitted());
    }

    #[test]
    fn test_defers_chan_alloc_to_last_fragment() {
        debug::setup_logging_verbose();

        let mut resource = get_default_resource();
        let chan_alloc = ChanAllocElement {
            alloc_type: ChanAllocType::Replace,
            ts_assigned: [false, true, false, false],
            ul_dl_assigned: UlDlAssignment::Both,
            clch_permission: false,
            cell_change_flag: false,
            carrier_num: 0,
            ext: None,
            mon_pattern: 0,
            frame18_mon_pattern: Some(0),
        };
        resource.chan_alloc_element = Some(chan_alloc.clone());

        let original_sdu_bitstr = &"00001111".repeat(60);
        let original_sdu = BitBuffer::from_bitstr(original_sdu_bitstr);

        // We'll reconstruct the bitstr from the fragments and check that it matches the original SDU
        let mut reconstructed_bitstr = String::new();

        let mut fragger = BsFragger::new(resource, original_sdu, None);

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let mut done = fragger.get_next_chunk(&mut mac_block);
        mac_block.seek(0);

        // Decode this chunk as a MacResource and check that the chan_alloc_element is not present
        let pdu = MacResource::from_bitbuf(&mut mac_block).unwrap();
        assert!(
            pdu.chan_alloc_element.is_none(),
            "Channel allocation element should be moved to last fragment"
        );

        // Fragmentation should start
        assert_eq!(pdu.length_ind, MAC_RESOURCE_LENGTH_FRAG_START);
        assert_eq!(pdu.fill_bits, false, "Fill bits should not be present in first fragment");

        // Append the SDU bits from the first fragment to the reconstructed bitstr
        mac_block.set_raw_start(mac_block.get_raw_pos());
        reconstructed_bitstr += &mac_block.to_bitstr();

        // Consume all fragments until the end
        while !done {
            mac_block = BitBuffer::new(SCH_HD_CAP);
            done = fragger.get_next_chunk(&mut mac_block);
            mac_block.seek(0);

            // If we're done, process the last fragment
            if done {
                break;
            }

            // Decode this chunk as a MacFragDl and check that the chan_alloc_element is not present
            let pdu = MacFragDl::from_bitbuf(&mut mac_block).unwrap();
            assert_eq!(
                pdu.fill_bits, false,
                "Fill bits should not be present in fragments before the last one"
            );

            // Append the SDU bits from this fragment to the reconstructed bitstr
            mac_block.set_raw_start(mac_block.get_raw_pos());
            reconstructed_bitstr += &mac_block.to_bitstr();
        }

        // Final chunk should be a MacEndDl with the chan_alloc_element present
        let pdu = MacEndDl::from_bitbuf(&mut mac_block).unwrap();
        assert!(
            pdu.chan_alloc_element.is_some(),
            "Channel allocation element should be present in last fragment"
        );

        // Should match the one on the original resource
        assert_eq!(
            pdu.chan_alloc_element.unwrap(),
            chan_alloc,
            "Channel allocation element in last fragment should match the one deferred from the original resource"
        );

        // Compute the length of the PDU - we can ignore the special-meaning Length Indications for this purpose
        let mut pdu_len_bits = pdu.length_ind as usize * 8;
        let num_fill_bits = if pdu.fill_bits {
            fillbits::removal::get_num_fill_bits(&mac_block, pdu_len_bits, false)
        } else {
            0
        };

        // Move the end of the MAC block to the end of the PDU, excluding the fill bits
        pdu_len_bits -= num_fill_bits;
        mac_block.set_raw_end(mac_block.get_raw_start() + pdu_len_bits);

        // Append the SDU bits from the last fragment to the reconstructed bitstr
        mac_block.set_raw_start(mac_block.get_raw_pos());
        reconstructed_bitstr += &mac_block.to_bitstr();

        // Check that the reconstructed bitstr matches the original SDU
        assert!(
            reconstructed_bitstr.eq(original_sdu_bitstr),
            "Reconstructed SDU should match the original SDU bitstr"
        );
    }

    #[test]
    fn test_does_not_fragment_simple_small_resource() {
        debug::setup_logging_verbose();

        // Resource containing a small SDU that should fit in a single MAC-RESOURCE PDU
        let resource = get_default_resource();
        let original_sdu_bitstr = "0000111100001111";
        let original_sdu = BitBuffer::from_bitstr(original_sdu_bitstr);

        let mut fragger = BsFragger::new(resource, original_sdu, None);

        let mut mac_block = BitBuffer::new(SCH_HD_CAP);
        let done = fragger.get_next_chunk(&mut mac_block);

        // Should be immediately complete
        assert!(done, "Small PDU should be transmitted in a single chunk");

        mac_block.seek(0);

        // Decode this chunk as a MacResource and check that the chan_alloc_element is not present
        let pdu = MacResource::from_bitbuf(&mut mac_block).unwrap();
        assert!(pdu.chan_alloc_element.is_none(), "Channel allocation element should not be present");

        // Fragmentation should not start
        assert_ne!(
            pdu.length_ind, MAC_RESOURCE_LENGTH_FRAG_START,
            "Length Indication should not indicate fragmentation for small PDU"
        );
    }

    #[test]
    fn test_defers_mac_resource_with_chan_alloc_when_not_enough_space() {
        debug::setup_logging_verbose();

        // Resource containing a small SDU that should fit in a single MAC-RESOURCE PDU, but will not once the channel allocation element is added
        // MAC header is 43 bits
        let mut resource = get_default_resource();

        // channel allocation element that uses 27 bits
        let chan_alloc = ChanAllocElement {
            alloc_type: ChanAllocType::Replace,
            ts_assigned: [false, true, false, false],
            ul_dl_assigned: UlDlAssignment::Both,
            clch_permission: false,
            cell_change_flag: false,
            carrier_num: 0,
            ext: None,
            mon_pattern: 0,
            frame18_mon_pattern: Some(0),
        };
        resource.chan_alloc_element = Some(chan_alloc.clone());

        // Pretend there are 80 bits of capacity left in this block
        let mut mac_block = BitBuffer::new(80);

        // 16 bits of SDU, so total PDU size is 43 + 27 + 16 = 86 bits (bigger than 80 bits remaining block)
        // Removing the 27 bit channel allocation allows the PDU to fit, however
        let original_sdu_bitstr = "0000111100001111";
        let original_sdu = BitBuffer::from_bitstr(original_sdu_bitstr);

        let mut fragger = BsFragger::new(resource, original_sdu, None);
        let done = fragger.get_next_chunk(&mut mac_block);

        // Should defer the entire PDU to the next frame
        assert!(!done, "Small PDU with channel allocation should be deferred to next frame");

        // The MAC block should be empty
        assert_eq!(
            mac_block.get_len_written(),
            0,
            "MAC block should be empty since the PDU was deferred"
        );

        // The channel allocation element should still be present in the resource
        assert!(
            fragger.resource.chan_alloc_element.is_some(),
            "Channel allocation element should still be present in the resource since the PDU was deferred"
        );
    }
}
