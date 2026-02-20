use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::*;
use tetra_core::{expect_value, let_field};

/// Clause 21.2.2.3 BL-DATA
#[derive(Debug, Clone)]
pub struct BlData {
    // 1
    pub has_fcs: bool,
    // 1
    pub ns: u8,
}

impl BlData {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // Parse 4-bit type, perform sanity checks
        let_field!(buf, llc_link_type, 1);
        expect_value!(llc_link_type, 0)?;
        let_field!(buf, has_fcs, 1);
        let_field!(buf, bl_pdu_type, 2);
        expect_value!(bl_pdu_type, 1)?;

        // Parse sequence number
        let_field!(buf, ns, 1);

        Ok(BlData {
            has_fcs: has_fcs != 0,
            ns: ns as u8,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant llc_link_type
        buf.write_bits(0, 1);
        buf.write_bits(self.has_fcs as u8 as u64, 1);
        // write required constant bl_pdu_type
        buf.write_bits(1, 2);
        buf.write_bits(self.ns as u64, 1);
    }
}

impl core::fmt::Display for BlData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "bl_data {{")?;
        write!(f, "  has_fcs: {}", self.has_fcs)?;
        write!(f, "  ns: {}", self.ns)?;
        write!(f, "}}")
    }
}
