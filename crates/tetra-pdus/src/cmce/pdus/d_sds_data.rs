use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-SDS-DATA PDU (Clause 14.7.1.10).
/// This PDU shall be for receiving user defined SDS data.
/// Response expected: -
/// Response to: -

// note 1: Shall be conditional on the value of Calling Party Type Identifier (CPTI): CPTI = 1: Calling Party SSI; CPTI = 2: Calling Party SSI + Calling Party Extension.
// note 2: Shall be conditional on the value of Short Data Type Identifier (SDTI): SDTI = 0: User Defined Data-1; SDTI = 1: User Defined Data-2; SDTI = 2: User Defined Data-3; SDTI = 3: Length Indicator + User Defined Data-4.
#[derive(Debug)]
pub struct DSdsData {
    /// Type1, 2 bits, Calling party type identifier
    pub calling_party_type_identifier: u8,
    /// Conditional 24 bits, See note 1, condition: calling_party_type_identifier == 1 || calling_party_type_identifier == 2
    pub calling_party_address_ssi: Option<u64>,
    /// Conditional 24 bits, See note 1, condition: calling_party_type_identifier == 1
    pub calling_party_extension: Option<u64>,
    /// Type1, 2 bits, Short data type identifier
    pub short_data_type_identifier: u8,
    /// Conditional 16 bits, See note 2, condition: short_data_type_identifier == 0
    pub user_defined_data_1: Option<u64>,
    /// Conditional 32 bits, See note 2, condition: short_data_type_identifier == 1
    pub user_defined_data_2: Option<u64>,
    /// Conditional 64 bits, See note 2, condition: short_data_type_identifier == 2
    pub user_defined_data_3: Option<u64>,
    /// Conditional 11 bits, See note 2, condition: short_data_type_identifier == 3
    pub length_indicator: Option<u64>,
    /// Conditional See note 2, condition: short_data_type_identifier == 3
    pub user_defined_data_4: Option<u64>,
    /// Type3, External subscriber number
    pub external_subscriber_number: Option<Type3FieldGeneric>,
    /// Type3, DM-MS address
    pub dm_ms_address: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DSdsData {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DSdsData)?;

        // Type1
        let calling_party_type_identifier = buffer.read_field(2, "calling_party_type_identifier")? as u8;
        // Conditional
        let calling_party_address_ssi = if calling_party_type_identifier == 1 || calling_party_type_identifier == 2 {
            Some(buffer.read_field(24, "calling_party_address_ssi")?)
        } else {
            None
        };
        // Conditional
        let calling_party_extension = if calling_party_type_identifier == 1 {
            Some(buffer.read_field(24, "calling_party_extension")?)
        } else {
            None
        };
        // Type1
        let short_data_type_identifier = buffer.read_field(2, "short_data_type_identifier")? as u8;
        // Conditional
        let user_defined_data_1 = if short_data_type_identifier == 0 {
            Some(buffer.read_field(16, "short_data_type_identifier")?)
        } else {
            None
        };
        // Conditional
        let user_defined_data_2 = if short_data_type_identifier == 1 {
            Some(buffer.read_field(32, "user_defined_data_2")?)
        } else {
            None
        };
        // Conditional
        let user_defined_data_3 = if short_data_type_identifier == 2 {
            Some(buffer.read_field(64, "user_defined_data_3")?)
        } else {
            None
        };
        // Conditional
        let length_indicator = if short_data_type_identifier == 3 {
            Some(buffer.read_field(11, "length_indicator")?)
        } else {
            None
        };
        // Conditional
        let user_defined_data_4 = if short_data_type_identifier == 3 {
            unimplemented!();
            Some(buffer.read_field(999, "user_defined_data_4")?)
        } else {
            None
        };

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

        Ok(DSdsData {
            calling_party_type_identifier,
            calling_party_address_ssi,
            calling_party_extension,
            short_data_type_identifier,
            user_defined_data_1,
            user_defined_data_2,
            user_defined_data_3,
            length_indicator,
            user_defined_data_4,
            external_subscriber_number,
            dm_ms_address,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DSdsData.into_raw(), 5);
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
        buffer.write_bits(self.short_data_type_identifier as u64, 2);
        // Conditional
        if let Some(ref value) = self.user_defined_data_1 {
            buffer.write_bits(*value, 16);
        }
        // Conditional
        if let Some(ref value) = self.user_defined_data_2 {
            buffer.write_bits(*value, 32);
        }
        // Conditional
        if let Some(ref value) = self.user_defined_data_3 {
            buffer.write_bits(*value, 64);
        }
        // Conditional
        if let Some(ref value) = self.length_indicator {
            buffer.write_bits(*value, 11);
        }
        // Conditional
        if let Some(ref _value) = self.user_defined_data_4 {
            unimplemented!();
            buffer.write_bits(*_value, 999);
        }

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

impl fmt::Display for DSdsData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DSdsData {{ calling_party_type_identifier: {:?} calling_party_address_ssi: {:?} calling_party_extension: {:?} short_data_type_identifier: {:?} user_defined_data_1: {:?} user_defined_data_2: {:?} user_defined_data_3: {:?} length_indicator: {:?} user_defined_data_4: {:?} external_subscriber_number: {:?} dm_ms_address: {:?} }}",
            self.calling_party_type_identifier,
            self.calling_party_address_ssi,
            self.calling_party_extension,
            self.short_data_type_identifier,
            self.user_defined_data_1,
            self.user_defined_data_2,
            self.user_defined_data_3,
            self.length_indicator,
            self.user_defined_data_4,
            self.external_subscriber_number,
            self.dm_ms_address,
        )
    }
}
