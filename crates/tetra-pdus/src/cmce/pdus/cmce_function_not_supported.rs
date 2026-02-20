use core::fmt;

use crate::cmce::enums::cmce_pdu_type_dl::CmcePduTypeDl;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the CMCE FUNCTION NOT SUPPORTED PDU (Clause 14.7.3.2).
/// This PDU may be sent by the MS or SwMI to indicate that the received PDU is not supported.
/// Response expected: -
/// Response to: Any individually addressed CMCE PDU

// note 1: This information element shall have value "CMCE FUNCTION NOT SUPPORTED" as specified in clause 14.8.28.
// note 2: This information element shall identify the PDU which contains the function which cannot be supported. The element shall have one of the values specified in clause 14.8.28.
// note 3: This information element shall be present if the value of the Call identifier present information element is "1"; this information element shall not be present if the value of the Call identifier present information element is "0" (zero).
// note 4: Element can have any value from 0 to 255₁₀; if non-zero, shall point to the first bit of the element in the received PDU which indicates the function that cannot be supported by the receiving entity. If zero, shall indicate that the PDU type itself (and hence the entire PDU specified by the "Not-supported PDU type" element) cannot be supported.
// note 5: Shall be conditional on the value of Function-not-supported pointer: if Function-not-supported pointer is non-zero, this element shall be present; if Function-not-supported pointer is zero, this element shall not be present.
// note 6: The total length of this element should be not less than the value of Function-not-supported pointer plus enough bits to identify the element in the received PDU which indicates the function that cannot be supported. This element shall not contain the PDU Type element of the received PDU because this is already specified by the "Not-supported PDU type" element (see note 2).
#[derive(Debug)]
pub struct CmceFunctionNotSupported {
    /// Type1, 5 bits, See note 2,
    pub not_supported_pdu_type: u8,
    /// Type1, 1 bits, Call identifier present
    pub call_identifier_present: bool,
    /// Conditional 14 bits, See note 3, condition: call_identifier_present == true
    pub call_identifier: Option<u64>,
    /// Type1, 8 bits, See note 4,
    pub function_not_supported_pointer: u8,
    /// Conditional 8 bits, See note 5, condition: function_not_supported_pointer != 0
    pub length_of_received_pdu_extract: Option<u64>,
    /// Conditional See notes 5 and 6, condition: function_not_supported_pointer != 0
    pub received_pdu_extract: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl CmceFunctionNotSupported {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::CmceFunctionNotSupported)?;

        // Type1
        let not_supported_pdu_type = buffer.read_field(5, "not_supported_pdu_type")? as u8;
        // Type1
        let call_identifier_present = buffer.read_field(1, "call_identifier_present")? != 0;
        // Conditional
        let call_identifier = if call_identifier_present {
            Some(buffer.read_field(14, "call_identifier")?)
        } else {
            None
        };
        // Type1
        let function_not_supported_pointer = buffer.read_field(8, "function_not_supported_pointer")? as u8;
        // Conditional
        let length_of_received_pdu_extract = if function_not_supported_pointer != 0 {
            Some(buffer.read_field(8, "length_of_received_pdu_extract")?)
        } else {
            None
        };
        // Conditional
        let received_pdu_extract = if function_not_supported_pointer != 0 {
            unimplemented!();
            Some(buffer.read_field(999, "received_pdu_extract")?)
        } else {
            None
        };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(CmceFunctionNotSupported {
            not_supported_pdu_type,
            call_identifier_present,
            call_identifier,
            function_not_supported_pointer,
            length_of_received_pdu_extract,
            received_pdu_extract,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::CmceFunctionNotSupported.into_raw(), 5);
        // Type1
        buffer.write_bits(self.not_supported_pdu_type as u64, 5);
        // Type1
        buffer.write_bits(self.call_identifier_present as u64, 1);
        // Conditional
        if let Some(ref value) = self.call_identifier {
            buffer.write_bits(*value, 14);
        }
        // Type1
        buffer.write_bits(self.function_not_supported_pointer as u64, 8);
        // Conditional
        if let Some(ref value) = self.length_of_received_pdu_extract {
            buffer.write_bits(*value, 8);
        }
        // Conditional
        if let Some(ref _value) = self.received_pdu_extract {
            unimplemented!();
            buffer.write_bits(*_value, 999);
        }
        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for CmceFunctionNotSupported {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "CmceFunctionNotSupported {{ not_supported_pdu_type: {:?} call_identifier_present: {:?} call_identifier: {:?} function_not_supported_pointer: {:?} length_of_received_pdu_extract: {:?} received_pdu_extract: {:?} }}",
            self.not_supported_pdu_type,
            self.call_identifier_present,
            self.call_identifier,
            self.function_not_supported_pointer,
            self.length_of_received_pdu_extract,
            self.received_pdu_extract,
        )
    }
}
