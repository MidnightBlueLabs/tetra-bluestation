use core::fmt;
use std::panic;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::umac::enums::{access_assign_dl_usage::AccessAssignDlUsage, access_assign_ul_usage::AccessAssignUlUsage};

#[derive(Debug, Clone, Copy)]
pub struct AccessField {
    // 2
    pub access_code: u8,
    // 4
    pub base_frame_len: u8,
}

/// Clause 21.4.7.2 ACCESS-ASSIGN
/// TODO FIXME technically not part of this SAP, but part of the MAC
#[derive(Debug)]
pub struct AccessAssign {
    // 2, kept for debugging purposes
    pub _header: u8,
    // 6
    pub dl_usage: AccessAssignDlUsage,
    pub ul_usage: AccessAssignUlUsage,

    // Three valid combinations:
    // - Only access_field (applies for both subslots)
    // - Both access_field1 and access_field2
    // - None (when dl and ul usage need to be sent)
    // pub access_field: Option<AccessField>,
    // pub access_field1: Option<AccessField>,
    // pub access_field2: Option<AccessField>,
    /// Populated when header == 0
    /// Provides access rights on UL subslot 1
    pub f1_af1: Option<AccessField>,
    // pub f1_dl_um: Option<AccessAssignDlUsage>,
    /// Populated when header == 0
    /// Provides access rights on UL subslot 2
    pub f2_af2: Option<AccessField>,

    /// Populated when header == 1 or 2
    /// Provides access rights on both UL subslots
    pub f2_af: Option<AccessField>,
    // pub f2_ul_um: Option<AccessAssignUlUsage>,
}

impl Default for AccessAssign {
    fn default() -> Self {
        AccessAssign {
            _header: 0,
            dl_usage: AccessAssignDlUsage::CommonControl,
            ul_usage: AccessAssignUlUsage::CommonOnly,
            f1_af1: None,
            // f1_dl_um: None,
            f2_af2: None,
            f2_af: None,
            // f2_ul_um: None
        }
    }
}

impl AccessAssign {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = AccessAssign {
            _header: buf.read_field(2, "_header")? as u8,
            ..Default::default()
        };

        let field1 = buf.read_field(6, "field1")? as u8;
        let field2 = buf.read_field(6, "field2")? as u8;

        match s._header {
            0 => {
                // DL common control
                // UL access rights - common only
                s.dl_usage = AccessAssignDlUsage::CommonControl;
                s.ul_usage = AccessAssignUlUsage::CommonOnly;

                s.f1_af1 = Some(AccessField {
                    access_code: (field1 >> 4) & 0x3,
                    base_frame_len: field1 & 0xF,
                });
                s.f2_af2 = Some(AccessField {
                    access_code: (field2 >> 4) & 0x3,
                    base_frame_len: field2 & 0xF,
                });
            }
            1 => {
                // DL defined by field1 usage marker
                // UL access rights - common and assigned
                s.dl_usage = AccessAssignDlUsage::from_usage_marker(field1);
                s.ul_usage = AccessAssignUlUsage::CommonAndAssigned;
                s.f2_af = Some(AccessField {
                    access_code: (field2 >> 4) & 0x3,
                    base_frame_len: field2 & 0xF,
                });
            }
            2 => {
                // DL defined by field1 usage marker
                // UL access rights - assigned only
                s.dl_usage = AccessAssignDlUsage::from_usage_marker(field1);
                s.ul_usage = AccessAssignUlUsage::AssignedOnly;
                s.f2_af = Some(AccessField {
                    access_code: (field2 >> 4) & 0x3,
                    base_frame_len: field2 & 0xF,
                });
            }
            3 => {
                // DL defined by field1 usage marker
                // UL defined by field2 usage marker
                s.dl_usage = AccessAssignDlUsage::from_usage_marker(field1);
                let ul_usage = AccessAssignUlUsage::from_usage_marker(field2);
                s.ul_usage = ul_usage.ok_or(PduParseErr::InvalidValue {
                    field: "ul_usage",
                    value: field2 as u64,
                })?;
            }
            _ => {
                panic!()
            }
        }

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // Safe fallback when a caller sets UL usage to CommonAndAssigned / AssignedOnly
        // but forgets to provide field2 (access field). Scheduler should still
        // set f2_af explicitly; this just prevents runtime panics.
        const DEFAULT_AF: AccessField = AccessField {
            access_code: 0,
            base_frame_len: 4,
        };

        if self.dl_usage == AccessAssignDlUsage::CommonControl && self.ul_usage == AccessAssignUlUsage::CommonOnly {
            assert!(
                self.f1_af1.is_some() && self.f2_af2.is_some(),
                "AccessAssign with CommonControl and CommonOnly must have both access fields defined"
            );
            assert!(
                self.f2_af.is_none(),
                "AccessAssign with CommonControl and CommonOnly must not have f2_af defined"
            );

            let header = 0;
            buf.write_bits(header as u64, 2);
            buf.write_bits(self.f1_af1.as_ref().unwrap().access_code as u64, 2);
            buf.write_bits(self.f1_af1.as_ref().unwrap().base_frame_len as u64, 4);
            buf.write_bits(self.f2_af2.as_ref().unwrap().access_code as u64, 2);
            buf.write_bits(self.f2_af2.as_ref().unwrap().base_frame_len as u64, 4);
        } else if self.ul_usage == AccessAssignUlUsage::CommonAndAssigned {
            let header = 1;
            buf.write_bits(header as u64, 2);

            let dl_usage = self.dl_usage.to_usage_marker();
            buf.write_bits(dl_usage as u64, 6);
            let af = self.f2_af.unwrap_or(DEFAULT_AF);
            buf.write_bits(af.access_code as u64, 2);
            buf.write_bits(af.base_frame_len as u64, 4);
        } else if self.ul_usage == AccessAssignUlUsage::AssignedOnly {
            let header = 2;
            buf.write_bits(header as u64, 2);
            let dl_usage = self.dl_usage.to_usage_marker();
            buf.write_bits(dl_usage as u64, 6);
            let af = self.f2_af.unwrap_or(DEFAULT_AF);
            buf.write_bits(af.access_code as u64, 2);
            buf.write_bits(af.base_frame_len as u64, 4);
        } else {
            // Both DL and UL usage given by usage markers
            let header = 3;
            buf.write_bits(header as u64, 2);

            let dl_usage = self.dl_usage.to_usage_marker();
            let ul_usage = self.ul_usage.to_usage_marker().unwrap();

            buf.write_bits(dl_usage as u64, 6);
            buf.write_bits(ul_usage as u64, 6);
        }
    }
}

impl fmt::Display for AccessAssign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "access_assign {{ dl_usage: {}, ul_usage: {}", self.dl_usage, self.ul_usage)?;
        if let Some(af) = &self.f2_af {
            write!(f, " access_field code: {} base_frame_len {}", af.access_code, af.base_frame_len)?;
        };
        if let Some(af1) = &self.f1_af1 {
            write!(f, " access_field1 code: {} base_frame_len {}", af1.access_code, af1.base_frame_len)?;
        };
        if let Some(af2) = &self.f2_af2 {
            write!(f, " access_field2 code: {} base_frame_len {}", af2.access_code, af2.base_frame_len)?;
        };
        write!(f, " }}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unallocated() {
        let bitstr = "11000000000000";
        let mut buf = BitBuffer::from_bitstr(bitstr);
        let mut bitarr = [0_u8; 14];
        let mut new_bitarr = [0_u8; 14];
        buf.to_bitarr(&mut bitarr);
        buf.seek(0);
        println!("buf: {}", buf.dump_bin());
        let pdu = AccessAssign::from_bitbuf(&mut buf).unwrap();
        println!("pdu: {:?}", pdu);
        let mut new_buf = BitBuffer::new(14);
        pdu.to_bitbuf(&mut new_buf);
        new_buf.seek(0);
        new_buf.to_bitarr(&mut new_bitarr);
        println!("new: {:?}", new_buf.dump_bin());
        assert_eq!(bitarr, new_bitarr);
    }

    #[test]
    fn test_commoncontrol() {
        let bitstr = "00001010001010";
        let mut buf = BitBuffer::from_bitstr(bitstr);
        let mut bitarr = [0_u8; 14];
        let mut new_bitarr = [0_u8; 14];
        buf.to_bitarr(&mut bitarr);
        buf.seek(0);
        println!("buf: {}", buf.dump_bin());
        let pdu = AccessAssign::from_bitbuf(&mut buf).unwrap();
        println!("pdu: {:?}", pdu);
        let mut new_buf = BitBuffer::new(14);
        pdu.to_bitbuf(&mut new_buf);
        new_buf.seek(0);
        new_buf.to_bitarr(&mut new_bitarr);
        println!("new: {:?}", new_buf.dump_bin());
        assert_eq!(bitarr, new_bitarr);
    }
}
