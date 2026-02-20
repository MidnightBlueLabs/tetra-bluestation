use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::*;
use tetra_core::{expect_value, let_field};

/// Clause 21.2.2.1 BL-ACK
#[derive(Debug, Clone)]
pub struct BlAck {
    // 1
    pub has_fcs: bool,
    // 1
    pub nr: u8,
    // pub tl_sdu: Option<BitBuffer>,
    // pub fcs: Option<u32>
}

impl BlAck {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // Parse 4-bit type, perform sanity checks
        let_field!(buf, llc_link_type, 1);
        expect_value!(llc_link_type, 0)?;
        let_field!(buf, has_fcs, 1);
        let_field!(buf, bl_pdu_type, 2);
        expect_value!(bl_pdu_type, 3)?;

        // Parse sequence number
        let_field!(buf, nr, 1);

        Ok(BlAck {
            has_fcs: has_fcs != 0,
            nr: nr as u8,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant llc_link_type
        buf.write_bits(0, 1);
        buf.write_bits(self.has_fcs as u8 as u64, 1);
        // write required constant bl_pdu_type
        buf.write_bits(3, 2);
        buf.write_bits(self.nr as u64, 1);
    }
}

impl fmt::Display for BlAck {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "bl_ack {{ has_fcs: {}, nr: {} }}", self.has_fcs, self.nr)
    }
}
