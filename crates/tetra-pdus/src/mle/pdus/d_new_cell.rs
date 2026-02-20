use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;

/// Representation of the D-NEW-CELL PDU (Clause 18.4.1.4.2).
/// Upon receipt from the SwMI the message shall inform the MS-MLE that it can select a new cell as previously indicated in the U-PREPARE or U-PREPARE-DA PDU.
/// Response expected: -
/// Response to: U-PREPARE/U-PREPARE-DA

// note 1: The SDU may carry an MM registration PDU which is used to forward register to a new cell during announced type 1 cell reselection or a D-OTAR CCK PROVIDE PDU which is used to identify the current CCK; it may also provide the future CCK for the LA which the MS has indicated in the U-OTAR CCK DEMAND PDU and whether the CCK provided is in use in other LAs or is used throughout the SwMI. The SDU is coded according to the MM protocol description. There shall be no P-bit in the PDU coding preceding the SDU information element.
#[derive(Debug)]
pub struct DNewCell {
    /// Type1, 2 bits, Channel command valid
    pub channel_command_valid: u8,
    /// Conditional SDU
    pub sdu: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl DNewCell {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::DNewCell)?;

        // Type1
        let channel_command_valid = buffer.read_field(2, "channel_command_valid")? as u8;
        // Exceptional case: obit required for SDU field.
        // SDU takes rest of slot, but still ends with 0-bit (closing obit)

        // obit designates presence of any further type2, type3 or type4 fields
        let obit = delimiters::read_obit(buffer)?;

        let sdu = if buffer.get_len_remaining() > 0 {
            Some(buffer.read_field(buffer.get_len_remaining() - 1, "sdu")?)
        } else {
            None
        };
        unimplemented!(); // read closing obit

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DNewCell {
            channel_command_valid,
            sdu,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeDl::DNewCell.into_raw(), 3);
        // Type1
        buffer.write_bits(self.channel_command_valid as u64, 2);
        // TODO FIXME: sdu handling
        // Conditional
        if let Some(ref value) = self.sdu {
            unimplemented!();
            buffer.write_bits(*value, 999);
        }
        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DNewCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DNewCell {{ channel_command_valid: {:?} sdu: {:?} }}",
            self.channel_command_valid, self.sdu,
        )
    }
}
