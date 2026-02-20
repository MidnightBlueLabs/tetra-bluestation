use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_ul::MmPduTypeUl;
use crate::mm::enums::status_uplink::StatusUplink;

/// Representation of the U-MM STATUS PDU (Clause 16.9.3.5.1).
/// The MS sends this message to the infrastructure to request or respond to a mode change.
/// Response expected: -/D-MM STATUS
/// Response to: -/D-MM STATUS

// note 1: This information element shall indicate the requested service or a response to a request and the sub-type of the U-MM STATUS PDU.
// note 2: This information element or set of information elements shall be as defined by the status uplink information element, refer to clauses 16.9.3.5.1 to 16.9.3.5.8.
// note 3: This Status uplink element indicates which sub-PDU this U-MM STATUS PDU contains; in case the receiving party does not support indicated function but recognizes this PDU structure, it should set the received value of Status uplink element to Not-supported sub PDU type element.
#[derive(Debug)]
pub struct UMmStatus {
    /// Type1, 6 bits, See notes 1 and 3,
    pub status_uplink: StatusUplink,
    /// Conditional See note 2,
    pub status_uplink_dependent_information: Option<u64>,
    pub status_uplink_dependent_information_len: Option<usize>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl UMmStatus {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeUl::UMmStatus)?;

        // Type1
        let val = buffer.read_field(6, "status_uplink")?;
        let result = StatusUplink::try_from(val);
        let status_uplink = match result {
            Ok(x) => x,
            Err(_) => {
                return Err(PduParseErr::InvalidValue {
                    field: "status_uplink",
                    value: val,
                });
            }
        };

        // // obit designates presence of any further type2, type3 or type4 fields
        // let mut obit = delimiters::read_obit(buffer)?;

        // // Read trailing obit (if not previously encountered)
        // obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        // if obit {
        //     return Err(PduParseErr::InvalidTrailingMbitValue);
        // }

        // We'll just get the remainder of this frame
        let bits_left = buffer.get_len_remaining();
        let status_uplink_dependent_information = if bits_left > 0 {
            Some(buffer.read_field(bits_left, "status_uplink_dependent_information")?)
        } else {
            None
        };

        Ok(UMmStatus {
            status_uplink,
            status_uplink_dependent_information,
            status_uplink_dependent_information_len: if bits_left > 0 { Some(bits_left) } else { None },
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeUl::UMmStatus.into_raw(), 4);
        // Type1
        buffer.write_bits(self.status_uplink as u64, 6);
        // Conditional
        if let Some(ref value) = self.status_uplink_dependent_information {
            // Unwrap should succeed as field must be present when optional status_uplink_dependent_information is present
            buffer.write_bits(*value, self.status_uplink_dependent_information_len.unwrap());
        }

        // Write terminating m-bit
        // delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for UMmStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "UMmStatus {{ status_uplink: {:?} status_uplink_dependent_information: {:?} }}",
            self.status_uplink, self.status_uplink_dependent_information,
        )
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_u_mm_status() {
        // Motorola MTH800. Hard to reproduce, but was emitted when EnergySavingMode is supplied in Location update but
        // the downlink response did not acknowledge it by setting the EnergySavingInformation
        debug::setup_logging_verbose();
        let test_vec = "00110000010010";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = UMmStatus::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        assert!(buf_in.get_len_remaining() == 0, "Buffer not fully consumed");

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);
    }
}
