use core::fmt;

use tetra_core::pdu_parse_error::PduParseErr;
use tetra_core::{BitBuffer, TdmaTime, assert_warn};

/// Clause 21.4.4.2
#[derive(Debug, Clone)]
pub struct MacSync {
    // 4
    pub system_code: u8,
    // 6
    pub colour_code: u8,
    // // 2
    // pub t: u8,
    // // 5
    // pub f: u8,
    // // 6
    // pub m: u8,
    pub time: TdmaTime,
    // 2
    pub sharing_mode: u8,
    // 3
    pub ts_reserved_frames: u8,
    // 1
    pub u_plane_dtx: bool,
    // 1
    pub frame_18_ext: bool,
    // 1
    // pub reserved: bool,
}

impl MacSync {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = MacSync {
            system_code: 0,
            colour_code: 0,
            // t: 0,
            // f: 0,
            // m: 0,
            time: TdmaTime::default(),
            sharing_mode: 0,
            ts_reserved_frames: 0,
            u_plane_dtx: false,
            frame_18_ext: false,
            // reserved: false,
        };

        s.system_code = buf.read_field(4, "system_code")? as u8;
        s.colour_code = buf.read_field(6, "colour_code")? as u8;
        let t = buf.read_field(2, "timeslot_number")? as u8 + 1;
        let f = buf.read_field(5, "frame_number")? as u8;
        let m = buf.read_field(6, "multiframe_number")? as u8;
        s.time = TdmaTime { t, f, m, h: 0 };
        s.sharing_mode = buf.read_field(2, "sharing_mode")? as u8;
        s.ts_reserved_frames = buf.read_field(3, "ts_reserved_frames")? as u8;
        s.u_plane_dtx = buf.read_field(1, "u_plane_dtx")? != 0;
        s.frame_18_ext = buf.read_field(1, "frame_18_ext")? != 0;
        assert_warn!(buf.read_field(1, "reserved")? == 0, "reserved bit not zero");

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.system_code as u64, 4);
        buf.write_bits(self.colour_code as u64, 6);
        buf.write_bits(self.time.t as u64 - 1, 2);
        buf.write_bits(self.time.f as u64, 5);
        buf.write_bits(self.time.m as u64, 6);
        buf.write_bits(self.sharing_mode as u64, 2);
        buf.write_bits(self.ts_reserved_frames as u64, 3);
        buf.write_bits(self.u_plane_dtx as u8 as u64, 1);
        buf.write_bits(self.frame_18_ext as u8 as u64, 1);
        buf.write_bits(0, 1);
    }
}

impl fmt::Display for MacSync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacSync {{ system_code: {}", self.system_code)?;
        write!(f, "  colour_code: {}", self.colour_code)?;
        write!(f, "  time: {:?}", self.time)?;
        write!(f, "  sharing_mode: {}", self.sharing_mode)?;
        write!(f, "  ts_reserved_frames: {}", self.ts_reserved_frames)?;
        write!(f, "  u_plane_dtx: {}", self.u_plane_dtx)?;
        write!(f, "  frame_18_ext: {}", self.frame_18_ext)?;
        write!(f, " }}")
    }
}
