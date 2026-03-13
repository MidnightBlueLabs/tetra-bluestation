use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, party_type_identifier::PartyTypeIdentifier, type3_elem_id::CmceType3ElemId};
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};
use tetra_saps::control::enums::sds_user_data::SdsUserData;

/// Representation of the D-SDS-DATA PDU (Clause 14.7.1.10).
/// This PDU shall be for receiving user defined SDS data.
/// Response expected: -
/// Response to: -

// note 1: Shall be conditional on the value of Calling Party Type Identifier (CPTI): CPTI = 1: Calling Party SSI; CPTI = 2: Calling Party SSI + Calling Party Extension.
// note 2: Shall be conditional on the value of Short Data Type Identifier (SDTI): SDTI = 0: User Defined Data-1; SDTI = 1: User Defined Data-2; SDTI = 2: User Defined Data-3; SDTI = 3: Length Indicator + User Defined Data-4.
#[derive(Debug)]
pub struct DSdsData {
    /// Type1, 2 bits, Calling party type identifier
    pub calling_party_type_identifier: PartyTypeIdentifier,
    /// Conditional 24 bits, See note 1, condition: calling_party_type_identifier == 1 || calling_party_type_identifier == 2
    pub calling_party_address_ssi: Option<u64>,
    /// Conditional 24 bits, See note 1, condition: calling_party_type_identifier == 2
    pub calling_party_extension: Option<u64>,
    /// Either type1, type2, type3 or type4 user data field.
    pub user_defined_data: SdsUserData,
    /// Type3, External subscriber number
    pub external_subscriber_number: Option<Type3FieldGeneric>,
    /// Type3, DM-MS address
    pub dm_ms_address: Option<Type3FieldGeneric>,
}

impl DSdsData {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DSdsData)?;

        // Type1
        let cpti_raw = buffer.read_field(2, "calling_party_type_identifier")?;
        let calling_party_type_identifier = PartyTypeIdentifier::try_from(cpti_raw).map_err(|_| PduParseErr::InvalidValue {
            field: "calling_party_type_identifier",
            value: cpti_raw,
        })?;
        // Conditional
        let calling_party_address_ssi =
            if calling_party_type_identifier == PartyTypeIdentifier::Ssi || calling_party_type_identifier == PartyTypeIdentifier::Tsi {
                Some(buffer.read_field(24, "calling_party_address_ssi")?)
            } else {
                None
            };
        // Conditional
        let calling_party_extension = if calling_party_type_identifier == PartyTypeIdentifier::Tsi {
            Some(buffer.read_field(24, "calling_party_extension")?)
        } else {
            None
        };

        // Type1
        let short_data_type_identifier = buffer.read_field(2, "short_data_type_identifier")? as u8;
        let user_defined_data = match short_data_type_identifier {
            0 => SdsUserData::Type1(buffer.read_field(16, "user_defined_data_1")? as u16),
            1 => SdsUserData::Type2(buffer.read_field(32, "user_defined_data_2")? as u32),
            2 => SdsUserData::Type3(buffer.read_field(64, "user_defined_data_3")?),
            3 => {
                let len_bits = buffer.read_field(11, "length_indicator")? as u16;
                let num_bytes = (len_bits + 7) / 8;
                let mut data = vec![0u8; num_bytes as usize];
                buffer
                    .read_bits_into_slice(len_bits as usize, &mut data)
                    .ok_or(PduParseErr::BufferEnded {
                        field: Some("user_defined_data_4"),
                    })?;
                SdsUserData::Type4(len_bits, data)
            }
            _ => unreachable!(),
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
            user_defined_data,
            external_subscriber_number,
            dm_ms_address,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DSdsData.into_raw(), 5);
        // Type1
        buffer.write_bits(self.calling_party_type_identifier.into_raw(), 2);
        // Conditional
        if let Some(ref value) = self.calling_party_address_ssi {
            buffer.write_bits(*value, 24);
        }
        // Conditional
        if let Some(ref value) = self.calling_party_extension {
            buffer.write_bits(*value, 24);
        }

        // Type1
        let short_data_type_identifier = self.user_defined_data.type_identifier();
        buffer.write_bits(short_data_type_identifier as u64, 2);

        match &self.user_defined_data {
            SdsUserData::Type1(value) => buffer.write_bits(*value as u64, 16),
            SdsUserData::Type2(value) => buffer.write_bits(*value as u64, 32),
            SdsUserData::Type3(value) => buffer.write_bits(*value, 64),
            SdsUserData::Type4(len_bits, data) => {
                buffer.write_bits(*len_bits as u64, 11);
                let full_bytes = (*len_bits as usize) / 8;
                let remaining_bits = len_bits % 8;
                for i in 0..full_bytes {
                    buffer.write_bits(data[i] as u64, 8);
                }
                if remaining_bits > 0 {
                    buffer.write_bits((data[full_bytes] >> (8 - remaining_bits)) as u64, remaining_bits as usize);
                }
            }
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
            "DSdsData {{ calling_party_type_identifier: {:?} calling_party_address_ssi: {:?} calling_party_extension: {:?} user_defined_data: {:?} external_subscriber_number: {:?} dm_ms_address: {:?} }}",
            self.calling_party_type_identifier,
            self.calling_party_address_ssi,
            self.calling_party_extension,
            self.user_defined_data,
            self.external_subscriber_number,
            self.dm_ms_address,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tetra_core::BitBuffer;

    fn round_trip(pdu: &DSdsData) -> DSdsData {
        let mut buf = BitBuffer::new_autoexpand(256);
        pdu.to_bitbuf(&mut buf).expect("serialize failed");
        buf.seek(0);
        DSdsData::from_bitbuf(&mut buf).expect("parse failed")
    }

    #[test]
    fn test_d_sds_data_sdti0_cpti1() {
        let pdu = DSdsData {
            calling_party_type_identifier: PartyTypeIdentifier::Ssi,
            calling_party_address_ssi: Some(1000001),
            calling_party_extension: None,
            user_defined_data: SdsUserData::Type1(0xABCD),
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.calling_party_type_identifier, PartyTypeIdentifier::Ssi);
        assert_eq!(parsed.calling_party_address_ssi, Some(1000001));
        assert_eq!(parsed.calling_party_extension, None);
        assert_eq!(parsed.user_defined_data, SdsUserData::Type1(0xABCD));
    }

    #[test]
    fn test_d_sds_data_sdti3_cpti1() {
        let payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0xCA];
        let pdu = DSdsData {
            calling_party_type_identifier: PartyTypeIdentifier::Ssi,
            calling_party_address_ssi: Some(2000002),
            calling_party_extension: None,
            user_defined_data: SdsUserData::Type4(40, payload.clone()), // 5 bytes = 40 bits
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.calling_party_type_identifier, PartyTypeIdentifier::Ssi);
        assert_eq!(parsed.calling_party_address_ssi, Some(2000002));
        assert_eq!(parsed.user_defined_data, SdsUserData::Type4(40, payload));
    }

    #[test]
    fn test_d_sds_data_cpti2_extension() {
        let pdu = DSdsData {
            calling_party_type_identifier: PartyTypeIdentifier::Tsi,
            calling_party_address_ssi: Some(3000003),
            calling_party_extension: Some(0x123456),
            user_defined_data: SdsUserData::Type1(0x1234),
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.calling_party_type_identifier, PartyTypeIdentifier::Tsi);
        assert_eq!(parsed.calling_party_address_ssi, Some(3000003));
        assert_eq!(parsed.calling_party_extension, Some(0x123456));
        assert_eq!(parsed.user_defined_data, SdsUserData::Type1(0x1234));
    }

    #[test]
    fn test_d_sds_data_cpti0() {
        let pdu = DSdsData {
            calling_party_type_identifier: PartyTypeIdentifier::Sna,
            calling_party_address_ssi: None,
            calling_party_extension: None,
            user_defined_data: SdsUserData::Type2(0xDEADBEEF),
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.calling_party_type_identifier, PartyTypeIdentifier::Sna);
        assert_eq!(parsed.calling_party_address_ssi, None);
        assert_eq!(parsed.calling_party_extension, None);
        assert_eq!(parsed.user_defined_data, SdsUserData::Type2(0xDEADBEEF));
    }
}
