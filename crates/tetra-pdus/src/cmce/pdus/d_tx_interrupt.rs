use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-TX INTERRUPT PDU (Clause 14.7.1.16).
/// This PDU shall be a message from the SwMI indicating that a permission to transmit has been withdrawn.
/// Response expected: -
/// Response to: -

// note 1: This information element is not used in this version of the present document and its value shall be set to "0".
// note 2: Shall be conditional on the value of Transmitting Party Type Identifier (TPTI): TPTI = 1; Transmitting Party SSI; TPTI = 2; Transmitting Party SSI + Transmitting Party Extension.
#[derive(Debug)]
pub struct DTxInterrupt {
    /// Type1, 14 bits, Call identifier
    pub call_identifier: u16,
    /// Type1, 2 bits, Transmission grant
    pub transmission_grant: u8,
    /// Type1, 1 bits, Transmission request permission
    /// Set to true to signal MSes they are allowed to send a U-TX DEMAND
    pub transmission_request_permission: bool,
    /// Type1, 1 bits, Encryption control
    pub encryption_control: bool,
    /// Type1, 1 bits, See note 1,
    pub reserved: bool,
    /// Type2, 6 bits, Notification indicator
    pub notification_indicator: Option<u64>,
    /// Type2, 2 bits, Transmitting party type identifier
    pub transmitting_party_type_identifier: Option<u64>,
    /// Type2, 24 bits, See note 2,
    pub transmitting_party_address_ssi: Option<u64>,
    /// Type2, 24 bits, See note 2,
    pub transmitting_party_extension: Option<u64>,
    /// Type3, External subscriber number
    pub external_subscriber_number: Option<Type3FieldGeneric>,
    /// Type3, Facility
    pub facility: Option<Type3FieldGeneric>,
    /// Type3, DM-MS address
    pub dm_ms_address: Option<Type3FieldGeneric>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DTxInterrupt {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DTxInterrupt)?;

        // Type1
        let call_identifier = buffer.read_field(14, "call_identifier")? as u16;
        // Type1
        let transmission_grant = buffer.read_field(2, "transmission_grant")? as u8;
        // Type1
        let transmission_request_permission = buffer.read_field(1, "transmission_request_permission")? != 0;
        // Type1
        let encryption_control = buffer.read_field(1, "encryption_control")? != 0;
        // Type1
        let reserved = buffer.read_field(1, "reserved")? != 0;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let notification_indicator = typed::parse_type2_generic(obit, buffer, 6, "notification_indicator")?;
        // Type2
        let transmitting_party_type_identifier = typed::parse_type2_generic(obit, buffer, 2, "transmitting_party_type_identifier")?;
        // Type2
        let transmitting_party_address_ssi = typed::parse_type2_generic(obit, buffer, 24, "transmitting_party_address_ssi")?;
        // Type2
        let transmitting_party_extension = typed::parse_type2_generic(obit, buffer, 24, "transmitting_party_extension")?;

        // Type3
        let external_subscriber_number = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::ExtSubscriberNum)?;

        // Type3
        let facility = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::Facility)?;

        // Type3
        let dm_ms_address = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::DmMsAddr)?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::Proprietary)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DTxInterrupt {
            call_identifier,
            transmission_grant,
            transmission_request_permission,
            encryption_control,
            reserved,
            notification_indicator,
            transmitting_party_type_identifier,
            transmitting_party_address_ssi,
            transmitting_party_extension,
            external_subscriber_number,
            facility,
            dm_ms_address,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DTxInterrupt.into_raw(), 5);
        // Type1
        buffer.write_bits(self.call_identifier as u64, 14);
        // Type1
        buffer.write_bits(self.transmission_grant as u64, 2);
        // Type1
        buffer.write_bits(self.transmission_request_permission as u64, 1);
        // Type1
        buffer.write_bits(self.encryption_control as u64, 1);
        // Type1
        buffer.write_bits(self.reserved as u64, 1);

        // Check if any optional field present and place o-bit
        let obit = self.notification_indicator.is_some()
            || self.transmitting_party_type_identifier.is_some()
            || self.transmitting_party_address_ssi.is_some()
            || self.transmitting_party_extension.is_some()
            || self.external_subscriber_number.is_some()
            || self.facility.is_some()
            || self.dm_ms_address.is_some()
            || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.notification_indicator, 6);

        // Type2
        typed::write_type2_generic(obit, buffer, self.transmitting_party_type_identifier, 2);

        // Type2
        typed::write_type2_generic(obit, buffer, self.transmitting_party_address_ssi, 24);

        // Type2
        typed::write_type2_generic(obit, buffer, self.transmitting_party_extension, 24);

        // Type3
        typed::write_type3_generic(obit, buffer, &self.external_subscriber_number, CmceType3ElemId::ExtSubscriberNum)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.facility, CmceType3ElemId::Facility)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.dm_ms_address, CmceType3ElemId::DmMsAddr)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.proprietary, CmceType3ElemId::Proprietary)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DTxInterrupt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DTxInterrupt {{ call_identifier: {:?} transmission_grant: {:?} transmission_request_permission: {:?} encryption_control: {:?} reserved: {:?} notification_indicator: {:?} transmitting_party_type_identifier: {:?} transmitting_party_address_ssi: {:?} transmitting_party_extension: {:?} external_subscriber_number: {:?} facility: {:?} dm_ms_address: {:?} proprietary: {:?} }}",
            self.call_identifier,
            self.transmission_grant,
            self.transmission_request_permission,
            self.encryption_control,
            self.reserved,
            self.notification_indicator,
            self.transmitting_party_type_identifier,
            self.transmitting_party_address_ssi,
            self.transmitting_party_extension,
            self.external_subscriber_number,
            self.facility,
            self.dm_ms_address,
            self.proprietary,
        )
    }
}
