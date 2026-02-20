use core::fmt;

use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

use crate::mle::enums::mle_pdu_type_ul::MlePduTypeUl;

/// Representation of the U-CHANNEL CLASS ADVICE PDU (Clause 18.4.1.4.8).
/// The message advises the SwMI of usable channel classes and the data priority of SN PDUs awaiting access to a packet data channel.
/// Response expected: -
/// Response to: -

// note 1: Shall indicate the number of “channel class identifier” information elements: 002 means one (4 bits); 012 means two (8 bits); 102 means three (12 bits); 112 means four (16 bits).
// note 2: Shall be present as many times as indicated by the “number of channel class identifiers” element; no P-bit preceding each element.
// note 3: There shall be no P-bit in the PDU coding preceding the “SDU” information element.
// note 4: If value is 0, the SwMI shall decode the SDU using the SNDCP protocol; if 1, using the protocol indicated by “protocol discriminator.”
// note 5: This instance of “protocol discriminator” shall be present only if “discriminator for SDU protocol present” is set to 1.
// note 6: If present, this instance of “protocol discriminator” indicates the SDU protocol.
#[derive(Debug)]
pub struct UChannelClassAdvice {
    /// Type1, 2 bits, See note 1,
    pub number_of_channel_class_identifiers: u8,
    /// Conditional 4 bits, Repeatable; see note 2,
    pub channel_class_identifier: Option<u64>,
    /// Type1, 1 bits, See note 4,
    pub discriminator_for_sdu_protocol_present: bool,
    /// Conditional 3 bits, See notes 5 and 6,
    pub protocol_discriminator: Option<u64>,
    /// Type2, 3 bits, Data priority
    pub data_priority: Option<u64>,
    /// Conditional See note 3,
    pub sdu: Option<u64>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
#[allow(unused_variables)]
impl UChannelClassAdvice {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(3, "pdu_type")?;
        expect_pdu_type!(pdu_type, MlePduTypeUl::UChannelClassAdvice)?;

        // Type1
        let number_of_channel_class_identifiers = buffer.read_field(2, "number_of_channel_class_identifiers")? as u8;
        // Conditional
        unimplemented!();
        let channel_class_identifier = if true { Some(0) } else { None };
        // Type1
        let discriminator_for_sdu_protocol_present = buffer.read_field(1, "discriminator_for_sdu_protocol_present")? != 0;
        // Conditional
        unimplemented!();
        let protocol_discriminator = if true { Some(0) } else { None };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let data_priority = typed::parse_type2_generic(obit, buffer, 3, "data_priority")?;
        // Conditional
        unimplemented!();
        let sdu = if obit { Some(0) } else { None };

        // Read trailing obit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(UChannelClassAdvice {
            number_of_channel_class_identifiers,
            channel_class_identifier,
            discriminator_for_sdu_protocol_present,
            protocol_discriminator,
            data_priority,
            sdu,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MlePduTypeUl::UChannelClassAdvice.into_raw(), 3);
        // Type1
        buffer.write_bits(self.number_of_channel_class_identifiers as u64, 2);
        // Conditional
        if let Some(ref value) = self.channel_class_identifier {
            buffer.write_bits(*value, 4);
        }
        // Type1
        buffer.write_bits(self.discriminator_for_sdu_protocol_present as u64, 1);
        // Conditional
        if let Some(ref value) = self.protocol_discriminator {
            buffer.write_bits(*value, 3);
        }

        // Check if any optional field present and place o-bit
        let obit = self.data_priority.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.data_priority, 3);

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

impl fmt::Display for UChannelClassAdvice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UChannelClassAdvice {{ number_of_channel_class_identifiers: {:?} channel_class_identifier: {:?} discriminator_for_sdu_protocol_present: {:?} protocol_discriminator: {:?} data_priority: {:?} sdu: {:?} }}",
            self.number_of_channel_class_identifiers,
            self.channel_class_identifier,
            self.discriminator_for_sdu_protocol_present,
            self.protocol_discriminator,
            self.data_priority,
            self.sdu,
        )
    }
}
