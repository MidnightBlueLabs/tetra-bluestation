use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

/// Clause 21.4.4.3 ACCESS-DEFINE
#[derive(Debug, Clone)]
pub struct AccessDefine {
    // 1
    pub common_or_assigned_control: bool,
    // 2
    pub access_code: u8,
    // 4
    pub imm: u8,
    // 4
    pub wt: u8,
    // 4
    pub nu: u8,
    // 1
    pub frame_len_factor: bool,
    // 4
    pub ts_pointer: u8,
    // 3
    pub min_pdu_prio: u8,
    // 2
    pub opt_field_flag: u8,
    // 16 opt
    pub subscriber_class: Option<u16>,
    // 24 opt
    pub gssi: Option<u32>,
}

impl AccessDefine {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = AccessDefine {
            common_or_assigned_control: false,
            access_code: 0,
            imm: 0,
            wt: 0,
            nu: 0,
            frame_len_factor: false,
            ts_pointer: 0,
            min_pdu_prio: 0,
            opt_field_flag: 0,
            subscriber_class: None,
            gssi: None,
        };

        // required constant mac_pdu_type
        assert!(buf.read_field(2, "mac_pdu_type")? == 2);
        // required constant broadcast_type
        assert!(buf.read_field(2, "broadcast_type")? == 1);
        s.common_or_assigned_control = buf.read_field(1, "common_or_assigned_control")? != 0;
        s.access_code = buf.read_field(2, "access_code")? as u8;
        s.imm = buf.read_field(4, "imm")? as u8;
        s.wt = buf.read_field(4, "wt")? as u8;
        s.nu = buf.read_field(4, "nu")? as u8;
        s.frame_len_factor = buf.read_field(1, "frame_len_factor")? != 0;
        s.ts_pointer = buf.read_field(4, "ts_pointer")? as u8;
        s.min_pdu_prio = buf.read_field(3, "min_pdu_prio")? as u8;
        s.opt_field_flag = buf.read_field(2, "opt_field_flag")? as u8;
        // TODO REVIEW: conditional read of subscriber_class
        if s.opt_field_flag == 1 {
            s.subscriber_class = Some(buf.read_field(16, "subscriber_class")? as u16);
        }
        // TODO REVIEW: conditional read of gssi
        if s.opt_field_flag == 2 {
            s.gssi = Some(buf.read_field(24, "gssi")? as u32);
        }
        // required constant FILLER
        assert!(buf.read_field(3, "filler")? == 4);

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(2, 2);
        // write required constant broadcast_type
        buf.write_bits(1, 2);
        buf.write_bits(self.common_or_assigned_control as u8 as u64, 1);
        buf.write_bits(self.access_code as u64, 2);
        buf.write_bits(self.imm as u64, 4);
        buf.write_bits(self.wt as u64, 4);
        buf.write_bits(self.nu as u64, 4);
        buf.write_bits(self.frame_len_factor as u8 as u64, 1);
        buf.write_bits(self.ts_pointer as u64, 4);
        buf.write_bits(self.min_pdu_prio as u64, 3);
        buf.write_bits(self.opt_field_flag as u64, 2);
        // TODO REVIEW: conditional write of subscriber_class
        if let Some(v) = self.subscriber_class {
            buf.write_bits(v as u64, 16);
        }
        // TODO REVIEW: conditional write of gssi
        if let Some(v) = self.gssi {
            buf.write_bits(v as u64, 24);
        }
        // write required constant FILLER
        buf.write_bits(4, 3);
    }
}

impl fmt::Display for AccessDefine {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "access_define {{ common_or_assigned_control: {}, access_code: {}, imm: {}, wt: {}, nu: {}, frame_len_factor: {}, ts_pointer: {}, min_pdu_prio: {}, opt_field_flag: {}",
            self.common_or_assigned_control,
            self.access_code,
            self.imm,
            self.wt,
            self.nu,
            self.frame_len_factor,
            self.ts_pointer,
            self.min_pdu_prio,
            self.opt_field_flag
        )?;

        if let Some(v) = self.subscriber_class {
            write!(f, "  subscriber_class: {}", v)?;
        };
        if let Some(v) = self.gssi {
            write!(f, "  gssi: {}", v)?;
        };
        write!(f, " }}")
    }
}
