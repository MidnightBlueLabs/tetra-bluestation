use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_ul::CmcePduTypeUl, party_type_identifier::PartyTypeIdentifier, type3_elem_id::CmceType3ElemId};
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the U-SDS-DATA PDU (Clause 14.7.2.8).
/// This PDU shall be for sending user defined SDS data.
/// Response expected: -
/// Response to: -

// note 1: This information element is used by SS-AS, refer to ETSI EN 300 392-12-8 [14].
// note 2: Shall be conditional on the value of Called Party Type Identifier (CPTI): CPTI=0 → Called Party SNA; CPTI=1 → Called Party SSI; CPTI=2 → Called Party SSI+Called Party Extension.
// note 3: Shall be conditional on the value of Short Data Type Identifier (SDTI): SDTI=0 → User Defined Data-1; SDTI=1 → User Defined Data-2; SDTI=2 → User Defined Data-3; SDTI=3 → Length indicator + User Defined Data-4.
// note 4: Any combination of address and user defined data type is allowed; recommended to choose the shortest appropriate user defined data type to fit one sub-slot when possible.
// note 5: The length of User Defined Data-4 is between 0 and 2 047 bits (longest recommended: 1 017 bits on basic link with Short SSI and FCS on π/4-DQPSK).
#[derive(Debug)]
pub struct USdsData {
    /// Type1, 4 bits, See note 1,
    pub area_selection: u8,
    /// Type1, 2 bits, Called party type identifier
    pub called_party_type_identifier: PartyTypeIdentifier,
    /// Conditional 8 bits, See note 2, condition: called_party_type_identifier == 0
    pub called_party_short_number_address: Option<u64>,
    /// Conditional 24 bits, See note 2, condition: called_party_type_identifier == 1 || called_party_type_identifier == 2
    pub called_party_ssi: Option<u64>,
    /// Conditional 24 bits, See note 2, condition: called_party_type_identifier == 2
    pub called_party_extension: Option<u64>,
    /// Type1, 2 bits, See note 4,
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
    pub user_defined_data_4: Option<Vec<u8>>,
    /// Type3, External subscriber number
    pub external_subscriber_number: Option<Type3FieldGeneric>,
    /// Type3, DM-MS address
    pub dm_ms_address: Option<Type3FieldGeneric>,
}

impl USdsData {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeUl::USdsData)?;

        // Type1
        let area_selection = buffer.read_field(4, "area_selection")? as u8;
        // Type1
        let cpti_raw = buffer.read_field(2, "called_party_type_identifier")?;
        let called_party_type_identifier = PartyTypeIdentifier::try_from(cpti_raw).map_err(|_| PduParseErr::InvalidValue {
            field: "called_party_type_identifier",
            value: cpti_raw,
        })?;
        // Conditional
        let called_party_short_number_address = if called_party_type_identifier == PartyTypeIdentifier::Sna {
            Some(buffer.read_field(8, "called_party_short_number_address")?)
        } else {
            None
        };
        // Conditional
        let called_party_ssi =
            if called_party_type_identifier == PartyTypeIdentifier::Ssi || called_party_type_identifier == PartyTypeIdentifier::Tsi {
                Some(buffer.read_field(24, "called_party_ssi")?)
            } else {
                None
            };
        // Conditional
        let called_party_extension = if called_party_type_identifier == PartyTypeIdentifier::Tsi {
            Some(buffer.read_field(24, "called_party_extension")?)
        } else {
            None
        };
        // Type1
        let short_data_type_identifier = buffer.read_field(2, "short_data_type_identifier")? as u8;
        // Conditional
        let user_defined_data_1 = if short_data_type_identifier == 0 {
            Some(buffer.read_field(16, "user_defined_data_1")?)
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
            let len_bits = length_indicator.unwrap() as usize;
            let num_bytes = (len_bits + 7) / 8;
            let mut data = vec![0u8; num_bytes];
            buffer.read_bits_into_slice(len_bits, &mut data).ok_or(PduParseErr::BufferEnded {
                field: Some("user_defined_data_4"),
            })?;
            Some(data)
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

        Ok(USdsData {
            area_selection,
            called_party_type_identifier,
            called_party_short_number_address,
            called_party_ssi,
            called_party_extension,
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
        buffer.write_bits(CmcePduTypeUl::USdsData.into_raw(), 5);
        // Type1
        buffer.write_bits(self.area_selection as u64, 4);
        // Type1
        buffer.write_bits(self.called_party_type_identifier.into_raw(), 2);
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
        if let Some(ref data) = self.user_defined_data_4 {
            let len_bits = self.length_indicator.unwrap() as usize;
            let full_bytes = len_bits / 8;
            let remaining_bits = len_bits % 8;
            for i in 0..full_bytes {
                buffer.write_bits(data[i] as u64, 8);
            }
            if remaining_bits > 0 {
                buffer.write_bits((data[full_bytes] >> (8 - remaining_bits)) as u64, remaining_bits);
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

impl fmt::Display for USdsData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "USdsData {{ area_selection: {:?} called_party_type_identifier: {:?} called_party_short_number_address: {:?} called_party_ssi: {:?} called_party_extension: {:?} short_data_type_identifier: {:?} user_defined_data_1: {:?} user_defined_data_2: {:?} user_defined_data_3: {:?} length_indicator: {:?} user_defined_data_4: {:?} external_subscriber_number: {:?} dm_ms_address: {:?} }}",
            self.area_selection,
            self.called_party_type_identifier,
            self.called_party_short_number_address,
            self.called_party_ssi,
            self.called_party_extension,
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

#[cfg(test)]
mod tests {
    use super::*;
    use tetra_core::BitBuffer;

    fn round_trip(pdu: &USdsData) -> USdsData {
        let mut buf = BitBuffer::new_autoexpand(256);
        pdu.to_bitbuf(&mut buf).expect("serialize failed");
        buf.seek(0);
        USdsData::from_bitbuf(&mut buf).expect("parse failed")
    }

    #[test]
    fn test_u_sds_data_sdti0_cpti1() {
        let pdu = USdsData {
            area_selection: 0,
            called_party_type_identifier: PartyTypeIdentifier::Ssi,
            called_party_short_number_address: None,
            called_party_ssi: Some(1000001),
            called_party_extension: None,
            short_data_type_identifier: 0,
            user_defined_data_1: Some(0xCAFE),
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: None,
            user_defined_data_4: None,
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.area_selection, 0);
        assert_eq!(parsed.called_party_type_identifier, PartyTypeIdentifier::Ssi);
        assert_eq!(parsed.called_party_ssi, Some(1000001));
        assert_eq!(parsed.called_party_extension, None);
        assert_eq!(parsed.short_data_type_identifier, 0);
        assert_eq!(parsed.user_defined_data_1, Some(0xCAFE));
    }

    #[test]
    fn test_u_sds_data_sdti3_cpti1() {
        let payload = vec![0x01, 0x02, 0x03];
        let pdu = USdsData {
            area_selection: 5,
            called_party_type_identifier: PartyTypeIdentifier::Ssi,
            called_party_short_number_address: None,
            called_party_ssi: Some(2000002),
            called_party_extension: None,
            short_data_type_identifier: 3,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: None,
            length_indicator: Some(24), // 3 bytes
            user_defined_data_4: Some(payload.clone()),
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.area_selection, 5);
        assert_eq!(parsed.called_party_ssi, Some(2000002));
        assert_eq!(parsed.short_data_type_identifier, 3);
        assert_eq!(parsed.length_indicator, Some(24));
        assert_eq!(parsed.user_defined_data_4, Some(payload));
    }

    #[test]
    fn test_u_sds_data_cpti0_sna() {
        let pdu = USdsData {
            area_selection: 0,
            called_party_type_identifier: PartyTypeIdentifier::Sna,
            called_party_short_number_address: Some(42),
            called_party_ssi: None,
            called_party_extension: None,
            short_data_type_identifier: 1,
            user_defined_data_1: None,
            user_defined_data_2: Some(0x12345678),
            user_defined_data_3: None,
            length_indicator: None,
            user_defined_data_4: None,
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.called_party_type_identifier, PartyTypeIdentifier::Sna);
        assert_eq!(parsed.called_party_short_number_address, Some(42));
        assert_eq!(parsed.called_party_ssi, None);
        assert_eq!(parsed.user_defined_data_2, Some(0x12345678));
    }

    #[test]
    fn test_u_sds_data_cpti2_extension() {
        let pdu = USdsData {
            area_selection: 0,
            called_party_type_identifier: PartyTypeIdentifier::Tsi,
            called_party_short_number_address: None,
            called_party_ssi: Some(3000003),
            called_party_extension: Some(0xABCDEF),
            short_data_type_identifier: 2,
            user_defined_data_1: None,
            user_defined_data_2: None,
            user_defined_data_3: Some(0x0102030405060708),
            length_indicator: None,
            user_defined_data_4: None,
            external_subscriber_number: None,
            dm_ms_address: None,
        };
        let parsed = round_trip(&pdu);
        assert_eq!(parsed.called_party_type_identifier, PartyTypeIdentifier::Tsi);
        assert_eq!(parsed.called_party_ssi, Some(3000003));
        assert_eq!(parsed.called_party_extension, Some(0xABCDEF));
        assert_eq!(parsed.short_data_type_identifier, 2);
        assert_eq!(parsed.user_defined_data_3, Some(0x0102030405060708));
    }
}
