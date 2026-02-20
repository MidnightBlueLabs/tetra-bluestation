use core::fmt::Display;

use tetra_core::{BitBuffer, assert_warn, pdu_parse_error::PduParseErr};

/// Clause 21.5.5 TS_COMMON_FRAMES
#[derive(Debug, Clone)]
pub struct TsCommonFrames {
    // 1
    pub f1: bool,
    // 1
    pub f2: bool,
    // 1
    pub f3: bool,
    // 1
    pub f4: bool,
    // 1
    pub f5: bool,
    // 1
    pub f6: bool,
    // 1
    pub f7: bool,
    // 1
    pub f8: bool,
    // 1
    pub f9: bool,
    // 1
    pub f10: bool,
    // 1
    pub f11: bool,
    // 1
    pub f12: bool,
    // 1
    pub f13: bool,
    // 1
    pub f14: bool,
    // 1
    pub f15: bool,
    // 1
    pub f16: bool,
    // 1
    pub f17: bool,
    // 1
    pub f18: bool,
    // 1 reserved
    // pub f19: bool,
    // 1 reserved
    // pub f20: bool,
}

impl TsCommonFrames {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let f1 = buf.read_field(1, "f1")? != 0;
        let f2 = buf.read_field(1, "f2")? != 0;
        let f3 = buf.read_field(1, "f3")? != 0;
        let f4 = buf.read_field(1, "f4")? != 0;
        let f5 = buf.read_field(1, "f5")? != 0;
        let f6 = buf.read_field(1, "f6")? != 0;
        let f7 = buf.read_field(1, "f7")? != 0;
        let f8 = buf.read_field(1, "f8")? != 0;
        let f9 = buf.read_field(1, "f9")? != 0;
        let f10 = buf.read_field(1, "f10")? != 0;
        let f11 = buf.read_field(1, "f11")? != 0;
        let f12 = buf.read_field(1, "f12")? != 0;
        let f13 = buf.read_field(1, "f13")? != 0;
        let f14 = buf.read_field(1, "f14")? != 0;
        let f15 = buf.read_field(1, "f15")? != 0;
        let f16 = buf.read_field(1, "f16")? != 0;
        let f17 = buf.read_field(1, "f17")? != 0;
        let f18 = buf.read_field(1, "f18")? != 0;
        let reserved = buf.read_field(2, "reserved")?;
        assert_warn!(reserved == 0, "reserved bits nonzero");

        Ok(TsCommonFrames {
            f1,
            f2,
            f3,
            f4,
            f5,
            f6,
            f7,
            f8,
            f9,
            f10,
            f11,
            f12,
            f13,
            f14,
            f15,
            f16,
            f17,
            f18,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.f1 as u8 as u64, 1);
        buf.write_bits(self.f2 as u8 as u64, 1);
        buf.write_bits(self.f3 as u8 as u64, 1);
        buf.write_bits(self.f4 as u8 as u64, 1);
        buf.write_bits(self.f5 as u8 as u64, 1);
        buf.write_bits(self.f6 as u8 as u64, 1);
        buf.write_bits(self.f7 as u8 as u64, 1);
        buf.write_bits(self.f8 as u8 as u64, 1);
        buf.write_bits(self.f9 as u8 as u64, 1);
        buf.write_bits(self.f10 as u8 as u64, 1);
        buf.write_bits(self.f11 as u8 as u64, 1);
        buf.write_bits(self.f12 as u8 as u64, 1);
        buf.write_bits(self.f13 as u8 as u64, 1);
        buf.write_bits(self.f14 as u8 as u64, 1);
        buf.write_bits(self.f15 as u8 as u64, 1);
        buf.write_bits(self.f16 as u8 as u64, 1);
        buf.write_bits(self.f17 as u8 as u64, 1);
        buf.write_bits(self.f18 as u8 as u64, 1);
        // buf.write_bits(self.f19 as u8 as u64, 1);
        // buf.write_bits(self.f20 as u8 as u64, 1);
        buf.write_bits(0, 2); // reserved bits
    }
}

impl Display for TsCommonFrames {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TsCommonFrames {{ {}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{}{} }}",
            self.f1,
            self.f2,
            self.f3,
            self.f4,
            self.f5,
            self.f6,
            self.f7,
            self.f8,
            self.f9,
            self.f10,
            self.f11,
            self.f12,
            self.f13,
            self.f14,
            self.f15,
            self.f16,
            self.f17,
            self.f18
        )
    }
}
