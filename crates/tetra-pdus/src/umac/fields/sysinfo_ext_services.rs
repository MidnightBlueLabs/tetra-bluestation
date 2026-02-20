use core::fmt;

use tetra_core::{BitBuffer, assert_warn, pdu_parse_error::PduParseErr};

/// Clause 21.4.4.1 SYSINFO Table 21.67 Extended Services and Part 7 Clause A.8.77 Security Information Element
#[derive(Debug, Clone)]
pub struct SysinfoExtendedServices {
    // 1
    pub auth_required: bool,
    // 1 opt
    pub class1_supported: bool,
    // 1 opt
    pub class2_supported: bool,
    // Not a field in the spec, but used in the code
    pub class3_supported: bool,
    // 5 opt
    pub sck_n: Option<u8>,
    // 1 opt
    pub dck_retrieval_during_cell_select: Option<bool>,
    // 1 opt
    pub dck_retrieval_during_cell_reselect: Option<bool>,
    // 1 opt
    pub linked_gck_crypto_periods: Option<bool>,
    // 2 opt
    pub short_gck_vn: Option<u8>,
    // 2
    pub sdstl_addressing_method: u8,
    // 1
    pub gck_supported: bool,
    // 2
    pub section: u8,
    // 7
    pub section_data: u8,
}

impl SysinfoExtendedServices {
    pub fn from_bitbuf(buf: &mut BitBuffer, aie_enabled: bool) -> Result<Self, PduParseErr> {
        // Read 3 bits from Security Information Element
        let auth_required = buf.read_field(1, "auth_required")? != 0;
        let (class1_supported, class2_supported, class3_supported) = if aie_enabled {
            let class1 = buf.read_field(1, "class1_supported")? == 1;
            let class2 = buf.read_field(1, "class2_supported")? == 0;
            let class3 = !class2;
            (class1, class2, class3)
        } else {
            let reserved = buf.read_field(2, "security_classes_reserved")?;
            assert_warn!(reserved == 0, "Security classes supported on AIE disabled network");
            (false, false, false)
        };

        // Read last 5 bits from Security Information Element
        let (sck_n, dck_retrieval_during_cell_select, dck_retrieval_during_cell_reselect, linked_gck_crypto_periods, short_gck_vn) =
            if class2_supported {
                let sck = Some(buf.read_field(5, "sck_n")? as u8);
                (sck, None, None, None, None)
            } else if class3_supported {
                let dck_select = Some(buf.read_field(1, "dck_retrieval_during_cell_select")? != 0);
                let dck_reselect = Some(buf.read_field(1, "dck_retrieval_during_cell_reselect")? != 0);
                let linked_gck = Some(buf.read_field(1, "linked_gck_crypto_periods")? != 0);
                let short_gck = Some(buf.read_field(2, "short_gck_vn")? as u8);
                (None, dck_select, dck_reselect, linked_gck, short_gck)
            } else {
                let reserved = buf.read_field(5, "security_info_reserved")?;
                assert_warn!(reserved == 0, "Security info present on AIE disabled network");
                (None, None, None, None, None)
            };

        // Read remaining 12 bits of Extended services broadcast information element
        let sdstl_addressing_method = buf.read_field(2, "sdstl_addressing_method")? as u8;
        let gck_supported = buf.read_field(1, "gck_supported")? != 0;
        let section = buf.read_field(2, "section")? as u8;
        let section_data = buf.read_field(7, "section_data")? as u8;

        Ok(SysinfoExtendedServices {
            auth_required,
            class1_supported,
            class2_supported,
            class3_supported,
            sck_n,
            dck_retrieval_during_cell_select,
            dck_retrieval_during_cell_reselect,
            linked_gck_crypto_periods,
            short_gck_vn,
            sdstl_addressing_method,
            gck_supported,
            section,
            section_data,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        assert!(
            !(self.class2_supported && self.class3_supported),
            "Both class2 and class3 supported"
        );

        // Write 3 bits to Security Information Element
        buf.write_bits(self.auth_required as u8 as u64, 1);
        buf.write_bits(self.class1_supported as u64, 1);
        buf.write_bits(self.class3_supported as u64, 1); // Writes 0 if class2 supported or none at all

        // Write remaining 5 bits to Security Information Element
        if self.class2_supported {
            buf.write_bits(self.sck_n.unwrap() as u64, 5);
        } else if self.class3_supported {
            buf.write_bits(self.dck_retrieval_during_cell_select.unwrap() as u8 as u64, 1);
            buf.write_bits(self.dck_retrieval_during_cell_reselect.unwrap() as u8 as u64, 1);
            buf.write_bits(self.linked_gck_crypto_periods.unwrap() as u8 as u64, 1);
            buf.write_bits(self.short_gck_vn.unwrap() as u64, 2);
        } else {
            buf.write_bits(0, 5); // Writes 0 if AIE disabled
        }

        // Write remaining 12 bits of Extended services broadcast information element
        buf.write_bits(self.sdstl_addressing_method as u64, 2);
        buf.write_bits(self.gck_supported as u8 as u64, 1);
        buf.write_bits(self.section as u64, 2);
        buf.write_bits(self.section_data as u64, 7);
    }
}

impl fmt::Display for SysinfoExtendedServices {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // preserve the original assertion
        assert!(
            !(self.class2_supported && self.class3_supported),
            "Both class2 and class3 supported"
        );

        writeln!(f, "tmb_sysinfo_extended_services {{")?;
        writeln!(f, "  auth_required: {}", self.auth_required)?;
        writeln!(
            f,
            "  Security classes: {}{}{}",
            if self.class1_supported { "1 " } else { "" },
            if self.class2_supported { "2 " } else { "" },
            if self.class3_supported { "3 " } else { "" },
        )?;

        if self.class2_supported {
            writeln!(f, "  sck_n: {}", self.sck_n.unwrap())?;
        } else if self.class3_supported {
            writeln!(
                f,
                "  dck_retrieval_during_cell_select: {}",
                self.dck_retrieval_during_cell_select.unwrap()
            )?;
            writeln!(
                f,
                "  dck_retrieval_during_cell_reselect: {}",
                self.dck_retrieval_during_cell_reselect.unwrap()
            )?;
            writeln!(f, "  linked_gck_crypto_periods: {}", self.linked_gck_crypto_periods.unwrap())?;
            writeln!(f, "  short_gck_vn: {}", self.short_gck_vn.unwrap())?;
        }

        writeln!(f, "  sdstl_addressing_method: {}", self.sdstl_addressing_method)?;
        writeln!(f, "  gck_supported: {}", self.gck_supported)?;
        writeln!(f, "  section: {}", self.section)?;
        writeln!(f, "  section_data: {}", self.section_data)?;
        write!(f, "}}")
    }
}
