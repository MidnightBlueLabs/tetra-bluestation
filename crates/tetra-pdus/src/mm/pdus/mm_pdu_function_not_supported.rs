use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};
use tetra_core::{expect_pdu_type, unimplemented_log};

use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;

/// Representation of the MM PDU/FUNCTION NOT SUPPORTED PDU (Clause 16.9.4.1).
/// This PDU may be sent by the MS/LS or SwMI to indicate that the received MM PDU or the function indicated in the PDU is not supported.
/// Response expected: -
/// Response to: Any individually addressed MM PDU

// note 1: This information element shall identify the received PDU which contains the function which cannot be supported.
// note 2: In case the receiving party recognizes the PDU and the PDU contains a sub-PDU field (like in U/M-MM STATUS PDU, U/D-OTAR, U/D-ENABLE, etc.) this element contains the element indicating which sub-PDU this is.
// note 3: The length of this element is indicated by the Length of the copied PDU element. This element is not present if the Length of the copied PDU element is not present.
// note 4: This element contains the received PDU beginning from and excluding the PDU type element.
#[derive(Debug)]
pub struct MmPduFunctionNotSupported {
    /// Type1, 4 bits, See note 1,
    pub not_supported_pdu_type: u8,
    /// Type2, See note 2. Holds (len_bits, value)
    pub not_supported_sub_pdu_type: Option<(usize, u64)>,
    // //// Type2, 8 bits, Length of the copied PDU
    // pub length_of_the_copied_pdu: Option<u64>,
    // /// Conditional See notes 3 and 4,
    // pub received_pdu_contents: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl MmPduFunctionNotSupported {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::MmPduFunctionNotSupported)?;

        // Type1
        let not_supported_pdu_type = buffer.read_field(4, "not_supported_pdu_type")? as u8;

        // obit designates presence of any further type2, type3 or type4 fields
        let obit = delimiters::read_obit(buffer)?;

        // Type2
        if !obit {
            return Ok(MmPduFunctionNotSupported {
                not_supported_pdu_type,
                not_supported_sub_pdu_type: None,
            });
        }

        unimplemented_log!("MmPduFunctionNotSupported parsing not fully implemented - please report to developers");
        Err(PduParseErr::NotImplemented {
            field: Some("MmPduFunctionNotSupported"),
        })
        // let not_supported_sub_pdu_type = typed::parse_type2_generic(obit, buffer, 999, "not_supported_sub_pdu_type")?;
        // // Type2
        // let length_of_the_copied_pdu = typed::parse_type2_generic(obit, buffer, 8, "length_of_the_copied_pdu")?;
        // // Conditional
        // unimplemented!(); let received_pdu_contents = if obit { Some(0) } else { None };

        // // Read trailing obit (if not previously encountered)
        // obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        // if obit {
        //     return Err(PduParseErr::InvalidTrailingMbitValue);
        // }

        // Ok(MmPduFunctionNotSupported {
        //     not_supported_pdu_type,
        //     not_supported_sub_pdu_type,
        //     length_of_the_copied_pdu,
        //     received_pdu_contents
        // })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeDl::MmPduFunctionNotSupported.into_raw(), 4);
        // Type1
        buffer.write_bits(self.not_supported_pdu_type as u64, 4);

        // Check if any optional field present and place o-bit
        let obit = self.not_supported_sub_pdu_type.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2

        // FIXME this is very messy - refactor later
        let (len, val) = self.not_supported_sub_pdu_type.unwrap(); // We know it's present
        typed::write_type2_generic(obit, buffer, Some(val), len);

        // let obit = self.not_supported_sub_pdu_type.is_some() || self.length_of_the_copied_pdu.is_some() ;
        // delimiters::write_obit(buffer, obit as u8);
        // if !obit { return Ok(()); }

        // // Type2
        // unimplemented!();
        //     typed::write_type2_generic(obit, buffer, self.not_supported_sub_pdu_type, 999);

        // // Type2
        // typed::write_type2_generic(obit, buffer, self.length_of_the_copied_pdu, 8);

        // // Conditional
        // if let Some(ref _value) = self.received_pdu_contents {
        //     unimplemented!();
        //     buffer.write_bits(*_value, 999);
        // }
        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for MmPduFunctionNotSupported {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // write!(f, "MmPduFunctionNotSupported {{ not_supported_pdu_type: {:?} not_supported_sub_pdu_type: {:?} length_of_the_copied_pdu: {:?} received_pdu_contents: {:?} }}",
        write!(
            f,
            "MmPduFunctionNotSupported {{ not_supported_pdu_type: {:?} not_supported_sub_pdu_type: {:?} }}",
            self.not_supported_pdu_type,
            self.not_supported_sub_pdu_type,
            // self.length_of_the_copied_pdu,
            // self.received_pdu_contents,
        )
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::debug;

    use crate::mm::enums::{mm_pdu_type_ul::MmPduTypeUl, status_uplink::StatusUplink};

    use super::*;

    #[test]
    fn test_mm_pdu_function_not_supported_parse() {
        // Self-generated vec!!!
        debug::setup_logging_verbose();
        let test_vec = "11110011110000010";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = MmPduFunctionNotSupported::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        assert!(buf_in.get_len_remaining() == 0, "Buffer not fully consumed");

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);
    }

    #[test]
    fn test_mm_pdu_function_not_support_write() {
        // Self-generated vec!!!
        // 1111 0011 1 10000010
        // |--|                     pdu type
        //      |--|                unsupported pdu type = UMmStatus (0x3)
        //           |              obit
        //             |-----|      unsupported sub pdu type = ChangeOfEnergySavingModeRequest
        //                     |    trailing obit

        debug::setup_logging_verbose();
        let pdu = MmPduFunctionNotSupported {
            not_supported_pdu_type: MmPduTypeUl::UMmStatus as u8,
            not_supported_sub_pdu_type: Some((6, StatusUplink::ChangeOfEnergySavingModeRequest.into())),
        };
        let mut test_buf = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut test_buf).unwrap();

        tracing::info!("Buf at end: {}", test_buf.dump_bin());
        let test_vec = "11110011110000010";

        assert_eq!(test_buf.to_bitstr(), test_vec);
    }
}
