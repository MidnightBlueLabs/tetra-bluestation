use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_ul::MlePduTypeUl;

/// Representation of the U-RESTORE PDU (Clause 18.4.1.4.7).
/// The message shall be sent by the MS-MLE, when restoration of the C-Plane towards a new cell is in progress.
/// Response expected: D-RESTORE-ACK/D-RESTORE-FAIL
/// Response to: -

// note 1: The element is present in the PDU if its value on the new cell is different from that on the old cell.
// note 2: When included, this element gives the value for the old cell.
// note 3: This PDU shall carry a CMCE U-CALL RESTORE PDU which shall be used to restore a call after cell reselection. There shall be no P-bit in the PDU coding preceding the "SDU" information element.
#[derive(Debug)]
pub struct URestore {
    /// Type2, 10 bits, See notes 1 and 2,
    pub mcc: Option<u64>,
    /// Type2, 14 bits, See notes 1 and 2,
    pub mnc: Option<u64>,
    /// Type2, 14 bits, See notes 1 and 2,
    pub la: Option<u64>,
    /// Conditional This PDU shall carry a CMCE U-CALL RESTORE PDU which shall be used to restore a call after cell reselection. The SDU is coded according to the CMCE protocol. There shall be no P-bit in the PDU coding preceding the "SDU" information element.,
    pub sdu: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl URestore {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeUl::URestore)?;

        // obit designates presence of any further type2, type3 or type4 fields
        let obit = delimiters::read_obit(buffer)?;

        // Type2
        let mcc = typed::parse_type2_generic(obit, buffer, 10, "mcc")?;
        // Type2
        let mnc = typed::parse_type2_generic(obit, buffer, 14, "mnc")?;
        // Type2
        let la = typed::parse_type2_generic(obit, buffer, 14, "la")?;
        // Conditional
        unimplemented!();
        let sdu = if obit { Some(0) } else { None };

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(URestore { mcc, mnc, la, sdu })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeUl::URestore.into_raw(), 3);

        // Check if any optional field present and place o-bit
        let obit = self.mcc.is_some() || self.mnc.is_some() || self.la.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.mcc, 10);

        // Type2
        typed::write_type2_generic(obit, buffer, self.mnc, 14);

        // Type2
        typed::write_type2_generic(obit, buffer, self.la, 14);

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

impl fmt::Display for URestore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "URestore {{ mcc: {:?} mnc: {:?} la: {:?} sdu: {:?} }}",
            self.mcc, self.mnc, self.la, self.sdu,
        )
    }
}
