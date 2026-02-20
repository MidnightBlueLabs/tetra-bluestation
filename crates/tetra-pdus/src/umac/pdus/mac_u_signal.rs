use core::fmt;

use tetra_core::BitBuffer;
use tetra_core::pdu_parse_error::PduParseErr;

/// Clause 21.4.5 MAC-U-SIGNAL
#[derive(Debug, Clone)]
pub struct MacUSignal {
    // 1
    pub second_half_stolen: bool,
}

impl MacUSignal {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        // required constant mac_pdu_type
        let mac_pdu_type = buf.read_field(2, "mac_pdu_type")?;
        assert!(mac_pdu_type == 3);
        let second_half_stolen = buf.read_field(1, "second_half_stolen")? != 0;

        Ok(MacUSignal { second_half_stolen })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        // write required constant mac_pdu_type
        buf.write_bits(3, 2);
        buf.write_bits(self.second_half_stolen as u8 as u64, 1);
    }
}

impl fmt::Display for MacUSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "mac_u_signal {{\n  second_half_stolen: {}\n}}\n", self.second_half_stolen,)
    }
}
