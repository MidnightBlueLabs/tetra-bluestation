use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

use crate::umac::fields::basic_slotgrant::BasicSlotgrant;

/// Clause 21.4.3.4 MAC-D-BLCK
#[derive(Debug, Clone)]
pub struct MacDBlck {
    // 1
    pub fill_bits: bool,
    // 2
    pub encryption_mode: u8,
    // 10
    pub event_label: u16,
    // 1
    pub imm_napping_permission: bool,
    // 1
    // pub slot_granting_flag: bool,
    // 8 opt
    pub slot_granting_element: Option<BasicSlotgrant>,
}

impl MacDBlck {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = MacDBlck {
            fill_bits: false,
            encryption_mode: 0,
            event_label: 0,
            imm_napping_permission: false,
            // slot_granting_flag: false,
            slot_granting_element: None,
        };

        // required constant mac_pdu_type
        assert!(buf.read_field(2, "mac_pdu_type")? == 3);
        // required constant pdu_subtype
        assert!(buf.read_field(1, "pdu_subtype")? == 0);

        s.fill_bits = buf.read_field(1, "fill_bits")? != 0;
        s.encryption_mode = buf.read_field(2, "encryption_mode")? as u8;
        s.event_label = buf.read_field(10, "event_label")? as u16;
        s.imm_napping_permission = buf.read_field(1, "imm_napping_permission")? != 0;

        let slot_granting_flag = buf.read_field(1, "slot_granting_flag")?;
        if slot_granting_flag == 1 {
            // 8-bit BasicSlotgrant element
            s.slot_granting_element = Some(BasicSlotgrant::from_bitbuf(buf)?);
        }

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type and pdu_subtype
        buf.write_bits(3, 2);
        buf.write_bits(0, 1);

        buf.write_bits(self.fill_bits as u8 as u64, 1);
        buf.write_bits(self.encryption_mode as u64, 2);
        buf.write_bits(self.event_label as u64, 10);
        buf.write_bits(self.imm_napping_permission as u8 as u64, 1);

        if let Some(v) = &self.slot_granting_element {
            buf.write_bits(1, 1);
            v.to_bitbuf(buf); // 8-bit BasicSlotgrant element
        } else {
            buf.write_bits(0, 1);
        }
    }
}

impl fmt::Display for MacDBlck {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MacDBlck {{ fill_bits: {}", self.fill_bits)?;
        write!(f, " encryption_mode: {}", self.encryption_mode)?;
        write!(f, " event_label: {}", self.event_label)?;
        write!(f, " imm_napping_permission: {}", self.imm_napping_permission)?;
        if let Some(v) = &self.slot_granting_element {
            write!(f, "  slot_granting_element: {}", v)?;
        }
        write!(f, " }}")
    }
}
