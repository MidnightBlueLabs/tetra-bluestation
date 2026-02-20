use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_ul::MlePduTypeUl;

/// Representation of the U-PREPARE PDU (Clause 18.4.1.4.6).
/// The message shall be sent on the serving cell to the SwMI by the MS-MLE, when preparation of cell reselection to a neighbour cell is in progress.
/// Response expected: D-NEW-CELL / D-NWRK-BROADCAST / D-PREPARE-FAIL
/// Response to: -

// note 1: The SDU may carry an MM registration PDU which is used to forward register to a new CA cell during announced type 1 cell reselection or a U-OTAR CCK DEMAND PDU which is used to request the Common Cipher Key (CCK) of the new cell. The SDU is coded according to the MM protocol description. There shall be no P-bit in the PDU coding preceding the SDU information element.
#[derive(Debug)]
pub struct UPrepare {
    /// Type2, 5 bits, Cell identifier CA
    pub cell_identifier_ca: Option<u64>,
    /// Conditional See note,
    pub sdu: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl UPrepare {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeUl::UPrepare)?;

        // obit designates presence of any further type2, type3 or type4 fields
        let obit = delimiters::read_obit(buffer)?;

        // Type2
        let cell_identifier_ca = typed::parse_type2_generic(obit, buffer, 5, "cell_identifier_ca")?;

        // Conditional
        unimplemented!();
        let sdu = if obit { Some(0) } else { None };

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(UPrepare { cell_identifier_ca, sdu })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeUl::UPrepare.into_raw(), 3);

        // Check if any optional field present and place o-bit
        let obit = self.cell_identifier_ca.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.cell_identifier_ca, 5);

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

impl fmt::Display for UPrepare {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UPrepare {{ cell_identifier_ca: {:?} sdu: {:?} }}",
            self.cell_identifier_ca, self.sdu,
        )
    }
}
