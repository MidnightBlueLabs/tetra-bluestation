use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

/// Clause 21.4.3.2 MAC-FRAG (downlink)
#[derive(Debug, Clone)]
pub struct MacFragDl {
    // 1
    pub fill_bits: bool,
}

impl MacFragDl {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        let mac_pdu_type = buf.read_field(2, "mac_pdu_type")?;
        assert!(mac_pdu_type == 1);
        // required constant pdu_subtype
        let pdu_subtype = buf.read_field(1, "pdu_subtype")?;
        assert!(pdu_subtype == 0);
        let fill_bits = buf.read_field(1, "fill_bits")? != 0;

        Ok(MacFragDl { fill_bits })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(1, 2);
        // write required constant pdu_subtype
        buf.write_bits(0, 1);
        buf.write_bits(self.fill_bits as u8 as u64, 1);
    }
}

impl fmt::Display for MacFragDl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacFragDl {{ fill_bits: {} }}", self.fill_bits)
    }
}
