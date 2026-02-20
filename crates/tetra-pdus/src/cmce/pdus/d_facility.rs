use core::fmt;

use crate::cmce::enums::cmce_pdu_type_dl::CmcePduTypeDl;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-FACILITY PDU (Clause 14.7.1.7).
/// This PDU shall be used to send call unrelated SS information.
/// Response expected: -
/// Response to: -

// note 1: Contents of this PDU shall be defined by SS protocols.
#[derive(Debug)]
pub struct DFacility {}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DFacility {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DFacility)?;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DFacility {})
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DFacility.into_raw(), 5);
        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DFacility {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DFacility {{ }}",)
    }
}
