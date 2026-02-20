use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

/// Clause 21.4.2.5 MAC-U-BLCK
#[derive(Debug, Clone)]
pub struct MacUBlck {
    // 1
    pub fill_bits: bool,
    // 1
    pub encrypted: bool,
    // 10
    pub event_label: u16,
    // 4
    pub reservation_req: u8, // WARNING don't use the regular ReservationRequirement enum, as there is a caveat in the highest two values
}

impl MacUBlck {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        let mac_pdu_type = buf.read_field(2, "mac_pdu_type")?;
        assert!(mac_pdu_type == 3);
        // required constant supp_pdu_subtype
        let supp_pdu_subtype = buf.read_field(1, "supp_pdu_subtype")?;
        assert!(supp_pdu_subtype == 0);
        let fill_bits = buf.read_field(1, "fill_bits")? != 0;
        let encrypted = buf.read_field(1, "encrypted")? != 0;
        let event_label = buf.read_field(10, "event_label")? as u16;
        let reservation_req = buf.read_field(4, "reservation_req")? as u8;

        Ok(MacUBlck {
            fill_bits,
            encrypted,
            event_label,
            reservation_req,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(3, 2);
        // write required constant supp_pdu_subtype
        buf.write_bits(0, 1);
        buf.write_bits(self.fill_bits as u8 as u64, 1);
        buf.write_bits(self.encrypted as u8 as u64, 1);
        buf.write_bits(self.event_label as u64, 10);
        buf.write_bits(self.reservation_req as u64, 4);
    }
}

impl fmt::Display for MacUBlck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacUBlck {{ fill_bits: {}", self.fill_bits)?;
        write!(f, "  encrypted: {}", self.encrypted)?;
        write!(f, "  addr: {}", self.event_label)?;
        write!(f, "  reservation_req: {}", self.reservation_req)?;
        write!(f, " }}")
    }
}
