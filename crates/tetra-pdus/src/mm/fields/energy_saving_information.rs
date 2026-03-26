use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::energy_saving_mode::EnergySavingMode;

/// 16.10.10 Energy saving information

#[derive(Debug, Clone)]
pub struct EnergySavingInformation {
    // 3
    pub energy_saving_mode: EnergySavingMode,
    // 5, when energy saving mode is "Stay alive" this field has no meaning and is set to 0
    pub frame_number: Option<u8>,
    // 6, when energy saving mode is "Stay alive" this field has no meaning and is set to 0
    pub multiframe_number: Option<u8>,
}

impl EnergySavingInformation {
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let val = buffer.read_field(3, "energy_saving_mode")? as u8;
        let energy_saving_mode = EnergySavingMode::try_from(val as u64).unwrap(); // Never fails

        let fn_val = buffer.read_field(5, "frame_number")? as u8;
        let mn_val = buffer.read_field(6, "multiframe_number")? as u8;

        // For StayAlive the spec says frame/multiframe fields "have no meaning";
        // parse them to advance the buffer, then discard.
        let (f, m) = if energy_saving_mode == EnergySavingMode::StayAlive {
            (None, None)
        } else {
            (Some(fn_val), Some(mn_val))
        };

        let s = EnergySavingInformation {
            energy_saving_mode,
            frame_number: f,
            multiframe_number: m,
        };

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) -> Result<(), PduParseErr> {
        buf.write_bits(self.energy_saving_mode as u64, 3);

        // Sanity check
        if self.energy_saving_mode == EnergySavingMode::StayAlive {
            if let Some(f) = self.frame_number {
                return Err(PduParseErr::InvalidValue {
                    field: "frame_number",
                    value: f as u64,
                });
            }
            if let Some(f) = self.multiframe_number {
                return Err(PduParseErr::InvalidValue {
                    field: "multiframe_number",
                    value: f as u64,
                });
            }
            buf.write_bits(0, 5 + 6);
        } else {
            if let Some(f) = self.frame_number {
                buf.write_bits(f as u64, 5);
            } else {
                return Err(PduParseErr::FieldNotPresent {
                    field: Some("frame_number"),
                });
            }
            if let Some(f) = self.multiframe_number {
                buf.write_bits(f as u64, 6);
            } else {
                return Err(PduParseErr::FieldNotPresent {
                    field: Some("multiframe_number"),
                });
            }
        }

        Ok(())
    }
}

impl fmt::Display for EnergySavingInformation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "EnergySavingInformation {{ energy_saving_mode: {:?} frame_number: {:?} multiframe_number: {:?} }}",
            self.energy_saving_mode, self.frame_number, self.multiframe_number,
        )
    }
}
