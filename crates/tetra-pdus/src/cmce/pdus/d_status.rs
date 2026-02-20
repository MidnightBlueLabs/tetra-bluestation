use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-STATUS PDU (Clause 14.7.1.11).
/// This PDU shall be the PDU for receiving a pre-coded status message.
/// Response expected: None
/// Response to: None

// Note 1: Shall be conditional on the value of Calling Party Type Identifier (CPTI): CPTI = 1 → include Calling Party SSI only; CPTI = 2 → include both SSI and Calling Party Extension.
#[derive(Debug)]
pub struct DStatus {
    /// Type1, 2 bits, Calling party type identifier
    pub calling_party_type_identifier: u8,
    /// Conditional 24 bits, Calling party address SSI condition: calling_party_type_identifier == 1 || calling_party_type_identifier == 2
    pub calling_party_address_ssi: Option<u64>,
    /// Conditional 24 bits, Calling party extension condition: calling_party_type_identifier == 2
    pub calling_party_extension: Option<u64>,
    /// Type1, 16 bits, Pre-coded status
    pub pre_coded_status: u16,
    /// Type3, External subscriber number
    pub external_subscriber_number: Option<Type3FieldGeneric>,
    /// Type3, DM-MS address
    pub dm_ms_address: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DStatus {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DStatus)?;

        // Type1
        let calling_party_type_identifier = buffer.read_field(2, "calling_party_type_identifier")? as u8;
        // Conditional
        let calling_party_address_ssi = if calling_party_type_identifier == 1 || calling_party_type_identifier == 2 {
            Some(buffer.read_field(24, "calling_party_address_ssi")?)
        } else {
            None
        };
        // Conditional
        let calling_party_extension = if calling_party_type_identifier == 2 {
            Some(buffer.read_field(24, "calling_party_extension")?)
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

        Ok(DStatus {
            calling_party_type_identifier,
            calling_party_address_ssi,
            calling_party_extension,
            pre_coded_status,
            external_subscriber_number,
            dm_ms_address,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DStatus.into_raw(), 5);
        // Type1
        buffer.write_bits(self.calling_party_type_identifier as u64, 2);
        // Conditional
        if let Some(ref value) = self.calling_party_address_ssi {
            buffer.write_bits(*value, 24);
        }
        // Conditional
        if let Some(ref value) = self.calling_party_extension {
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

impl fmt::Display for DStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DStatus {{ calling_party_type_identifier: {:?} calling_party_address_ssi: {:?} calling_party_extension: {:?} pre_coded_status: {:?} external_subscriber_number: {:?} dm_ms_address: {:?} }}",
            self.calling_party_type_identifier,
            self.calling_party_address_ssi,
            self.calling_party_extension,
            self.pre_coded_status,
            self.external_subscriber_number,
            self.dm_ms_address,
        )
    }
}
