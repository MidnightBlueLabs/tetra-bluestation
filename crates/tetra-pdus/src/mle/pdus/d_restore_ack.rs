use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;

/// Representation of the D-RESTORE-ACK PDU (Clause 18.4.1.4.4).
/// Upon receipt from the SwMI, the message shall indicate to the MS-MLE an acknowledgement of the C-Plane restoration on the new selected cell.
/// Response expected: -
/// Response to: U-RESTORE

// note 1: This PDU shall carry a CMCE D-CALL RESTORE PDU which can be used to restore a call after cell reselection. The SDU is coded according to the CMCE protocol description. There shall be no P-bit in the PDU coding preceding the SDU information element.
#[derive(Debug)]
pub struct DRestoreAck {
    /// Conditional See note,
    pub sdu: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl DRestoreAck {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::DRestoreAck)?;

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

        Ok(DRestoreAck { sdu })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeDl::DRestoreAck.into_raw(), 3);
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

impl fmt::Display for DRestoreAck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DRestoreAck {{ sdu: {:?} }}", self.sdu)
    }
}
