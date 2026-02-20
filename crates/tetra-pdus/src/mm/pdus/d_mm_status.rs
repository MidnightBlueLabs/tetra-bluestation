use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;

/// Representation of the D-MM STATUS PDU (Clause 16.9.2.5.1).
/// The infrastructure sends this message to the MS to request or indicate/reject a change of an operation mode.
/// Response expected: -/U-MM STATUS
/// Response to: -/U-MM STATUS

// note 1: This information element shall indicate the requested service or a response to a request and the sub-type of the D-MM STATUS PDU.
// note 2: This information element or set of information elements shall be as defined by the status downlink information element, refer to clauses 16.9.2.5.1 to 16.9.2.5.7.
// note 3: This Status downlink element indicates which sub-PDU this D-MM STATUS PDU contains. If the receiving party does not support the indicated function but recognizes the PDU structure, it should set the value to Not-supported sub-PDU type element.
#[derive(Debug)]
pub struct DMmStatus {
    /// Type1, 6 bits, See notes 1 and 3,
    pub status_downlink: u8,
    /// Conditional See note 2,
    pub status_downlink_dependent_information: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl DMmStatus {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::DMmStatus)?;

        // Type1
        let status_downlink = buffer.read_field(6, "status_downlink")? as u8;
        // Conditional
        unimplemented!();
        let status_downlink_dependent_information = if true { Some(0) } else { None };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DMmStatus {
            status_downlink,
            status_downlink_dependent_information,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeDl::DMmStatus.into_raw(), 4);
        // Type1
        buffer.write_bits(self.status_downlink as u64, 6);
        // Conditional
        if let Some(ref value) = self.status_downlink_dependent_information {
            unimplemented!();
            buffer.write_bits(*value, 999);
        }
        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DMmStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DMmStatus {{ status_downlink: {:?} status_downlink_dependent_information: {:?} }}",
            self.status_downlink, self.status_downlink_dependent_information,
        )
    }
}
