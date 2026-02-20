use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

use crate::umac::fields::basic_slotgrant::BasicSlotgrant;
use crate::umac::fields::channel_allocation::ChanAllocElement;

/// Clause 21.4.3.3 MAC-END (downlink)
#[derive(Debug, Clone)]
pub struct MacEndDl {
    // 1
    pub fill_bits: bool,
    // 1
    pub pos_of_grant: u8,
    /// 6 bits, depending on modulation some interesting length calculation may need to be applied
    pub length_ind: u8,
    // 1
    // pub slot_granting_flag: bool,
    // 8 opt
    pub slot_granting_element: Option<BasicSlotgrant>,
    // 1
    // pub chan_alloc_flag: bool,
    // 999 opt
    pub chan_alloc_element: Option<ChanAllocElement>,
}

impl MacEndDl {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = MacEndDl {
            fill_bits: false,
            pos_of_grant: 0,
            length_ind: 0,
            slot_granting_element: None,
            chan_alloc_element: None,
        };

        // required constant mac_pdu_type
        assert!(buf.read_field(2, "mac_pdu_type")? == 1);
        // required constant pdu_subtype
        assert!(buf.read_field(1, "pdu_subtype")? == 1);
        s.fill_bits = buf.read_field(1, "fill_bits")? != 0;
        s.pos_of_grant = buf.read_field(1, "pos_of_grant")? as u8;
        s.length_ind = buf.read_field(6, "length_ind")? as u8;

        let slot_granting_flag = buf.read_field(1, "slot_granting_flag")?;
        if slot_granting_flag == 1 {
            // Read 8-bit BasicSlotgrant element
            s.slot_granting_element = Some(BasicSlotgrant::from_bitbuf(buf)?);
        }

        let chan_alloc_flag = buf.read_field(1, "chan_alloc_flag")?;
        if chan_alloc_flag == 1 {
            s.chan_alloc_element = Some(ChanAllocElement::from_bitbuf(buf)?);
        }

        Ok(s)
    }

    pub fn compute_hdr_len(has_slotgrant: bool, has_chanalloc: bool) -> usize {
        assert!(!has_chanalloc, "unimplemented");
        2 + 1 + 1 + 1 + 6 + 1 + (if has_slotgrant { 6 } else { 0 }) + 1
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type and pdu_subtype
        buf.write_bits(1, 2);
        buf.write_bits(1, 1);

        buf.write_bits(self.fill_bits as u8 as u64, 1);
        buf.write_bits(self.pos_of_grant as u64, 1);
        buf.write_bits(self.length_ind as u64, 6);

        if let Some(v) = &self.slot_granting_element {
            buf.write_bits(1, 1);
            // Write 8-bit BasicSlotgrant element
            v.to_bitbuf(buf);
        } else {
            buf.write_bits(0, 1);
        }

        if let Some(v) = &self.chan_alloc_element {
            buf.write_bits(1, 1); // Chan alloc flag
            v.to_bitbuf(buf);
        } else {
            buf.write_bits(0, 1);
        }
    }
}

impl fmt::Display for MacEndDl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacEndDl {{ fill_bits: {}", self.fill_bits)?;
        write!(f, "  pos_of_grant: {}", self.pos_of_grant)?;
        write!(f, "  length_ind: {}", self.length_ind)?;
        if let Some(v) = &self.slot_granting_element {
            write!(f, "  slot_granting_element: {}", v)?;
        }
        if let Some(v) = &self.chan_alloc_element {
            write!(f, "  chan_alloc_element: {:?}", v)?;
        }
        write!(f, "}}")
    }
}
