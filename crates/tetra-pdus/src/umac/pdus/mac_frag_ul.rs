use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

/// Clause 21.4.2.4 MAC-FRAG (uplink)
#[derive(Debug, Clone)]
pub struct MacFragUl {
    // 1
    pub fill_bits: bool,
}

impl MacFragUl {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        let mac_pdu_type = buf.read_field(2, "mac_pdu_type")?;
        assert!(mac_pdu_type == 1);
        // required constant pdu_subtype
        let pdu_subtype = buf.read_field(1, "pdu_subtype")?;
        assert!(pdu_subtype == 0);
        let fill_bits = buf.read_field(1, "fill_bits")? != 0;

        Ok(MacFragUl { fill_bits })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(1, 2);
        // write required constant pdu_subtype
        buf.write_bits(0, 1);
        buf.write_bits(self.fill_bits as u8 as u64, 1);
    }
}

impl fmt::Display for MacFragUl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacFragUl {{ fill_bits: {} }}", self.fill_bits)
    }
}
