use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_ul::CmcePduTypeUl, type3_elem_id::CmceType3ElemId};
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the U-STATUS PDU (Clause 14.7.2.7).
/// This PDU shall be used for sending a pre-coded status message.
/// Response expected: -
/// Response to: -

// note 1: This information element is used by SS-AS, refer to ETSI EN 300 392-12-8 [14].
// note 2: Shall be conditional on the value of Called Party Type Identifier (CPTI): CPTI = 0 → Called Party SNA (see ETS 300 392-12-7 [13]); CPTI = 1 → Called Party SSI; CPTI = 2 → Called Party SSI + Called Party Extension.
#[derive(Debug)]
pub struct UStatus {
    /// Type1, 4 bits, See note 1,
    pub area_selection: u8,
    /// Type1, 2 bits, Short/SSI/TSI,
    pub called_party_type_identifier: u8,
    /// Conditional 8 bits, See note 2, condition: called_party_type_identifier == 0
    pub called_party_short_number_address: Option<u64>,
    /// Conditional 24 bits, See note 2, condition: called_party_type_identifier == 1 || called_party_type_identifier == 2
    pub called_party_ssi: Option<u64>,
    /// Conditional 24 bits, See note 2, condition: called_party_type_identifier == 2
    pub called_party_extension: Option<u64>,
    /// Type1, 16 bits, Pre-coded status
    pub pre_coded_status: u16,
    /// Type3, External subscriber number
    pub external_subscriber_number: Option<Type3FieldGeneric>,
    /// Type3, DM-MS address
    pub dm_ms_address: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl UStatus {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeUl::UStatus)?;

        // Type1
        let area_selection = buffer.read_field(4, "area_selection")? as u8;
        // Type1
        let called_party_type_identifier = buffer.read_field(2, "called_party_type_identifier")? as u8;
        // Conditional
        let called_party_short_number_address = if called_party_type_identifier == 0 {
            Some(buffer.read_field(8, "called_party_short_number_address")?)
        } else {
            None
        };
        // Conditional
        let called_party_ssi = if called_party_type_identifier == 1 || called_party_type_identifier == 2 {
            Some(buffer.read_field(24, "called_party_ssi")?)
        } else {
            None
        };
        // Conditional
        let called_party_extension = if called_party_type_identifier == 2 {
            Some(buffer.read_field(24, "called_party_extension")?)
        } else {
            None
        };
        // Type1
        let pre_coded_status = buffer.read_field(16, "pre_coded_status")? as u16;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type3
        let external_subscriber_number = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::ExtSubscriberNum)?;

        // Type3
        let dm_ms_address = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::DmMsAddr)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(UStatus {
            area_selection,
            called_party_type_identifier,
            called_party_short_number_address,
            called_party_ssi,
            called_party_extension,
            pre_coded_status,
            external_subscriber_number,
            dm_ms_address,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeUl::UStatus.into_raw(), 5);
        // Type1
        buffer.write_bits(self.area_selection as u64, 4);
        // Type1
        buffer.write_bits(self.called_party_type_identifier as u64, 2);
        // Conditional
        if let Some(ref value) = self.called_party_short_number_address {
            buffer.write_bits(*value, 8);
        }
        // Conditional
        if let Some(ref value) = self.called_party_ssi {
            buffer.write_bits(*value, 24);
        }
        // Conditional
        if let Some(ref value) = self.called_party_extension {
            buffer.write_bits(*value, 24);
        }
        // Type1
        buffer.write_bits(self.pre_coded_status as u64, 16);

        // Check if any optional field present and place o-bit
        let obit = self.external_subscriber_number.is_some() || self.dm_ms_address.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type3
        typed::write_type3_generic(obit, buffer, &self.external_subscriber_number, CmceType3ElemId::ExtSubscriberNum)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.dm_ms_address, CmceType3ElemId::DmMsAddr)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for UStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "UStatus {{ area_selection: {:?} called_party_type_identifier: {:?} called_party_short_number_address: {:?} called_party_ssi: {:?} called_party_extension: {:?} pre_coded_status: {:?} external_subscriber_number: {:?} dm_ms_address: {:?} }}",
            self.area_selection,
            self.called_party_type_identifier,
            self.called_party_short_number_address,
            self.called_party_ssi,
            self.called_party_extension,
            self.pre_coded_status,
            self.external_subscriber_number,
            self.dm_ms_address,
        )
    }
}
