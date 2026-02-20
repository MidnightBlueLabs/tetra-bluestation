use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_dl::MlePduTypeDl;

/// Representation of the D-CHANNEL RESPONSE PDU (Clause 18.4.1.4.5a).
/// The message shall be sent by the SwMI in response to an MS request for an assigned channel replacement.
/// Response expected: -
/// Response to: U-CHANNEL REQUEST

// note 1: In the present document, this element shall not be included.
#[derive(Debug)]
pub struct DChannelResponse {
    /// Type1, 1 bits, Channel response type
    pub channel_response_type: bool,
    /// Type1, 3 bits, Reason for the channel request
    pub reason_for_the_channel_request: u8,
    /// Type1, 4 bits, Channel request retry delay
    pub channel_request_retry_delay: u8,
    /// Type2, 8 bits, See note,
    pub reserved1: Option<u64>,
    /// Type2, 8 bits, See note,
    pub reserved2: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DChannelResponse {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeDl::DChannelResponse)?;

        // Type1
        let channel_response_type = buffer.read_field(1, "channel_response_type")? != 0;
        // Type1
        let reason_for_the_channel_request = buffer.read_field(3, "reason_for_the_channel_request")? as u8;
        // Type1
        let channel_request_retry_delay = buffer.read_field(4, "channel_request_retry_delay")? as u8;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let reserved1 = typed::parse_type2_generic(obit, buffer, 8, "reserved1")?;
        // Type2
        let reserved2 = typed::parse_type2_generic(obit, buffer, 8, "reserved2")?;

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DChannelResponse {
            channel_response_type,
            reason_for_the_channel_request,
            channel_request_retry_delay,
            reserved1,
            reserved2,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeDl::DChannelResponse.into_raw(), 3);
        // Type1
        buffer.write_bits(self.channel_response_type as u64, 1);
        // Type1
        buffer.write_bits(self.reason_for_the_channel_request as u64, 3);
        // Type1
        buffer.write_bits(self.channel_request_retry_delay as u64, 4);

        // Check if any optional field present and place o-bit
        let obit = self.reserved1.is_some() || self.reserved2.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.reserved1, 8);

        // Type2
        typed::write_type2_generic(obit, buffer, self.reserved2, 8);

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DChannelResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DChannelResponse {{ channel_response_type: {:?} reason_for_the_channel_request: {:?} channel_request_retry_delay: {:?} reserved1: {:?} reserved2: {:?} }}",
            self.channel_response_type,
            self.reason_for_the_channel_request,
            self.channel_request_retry_delay,
            self.reserved1,
            self.reserved2,
        )
    }
}
