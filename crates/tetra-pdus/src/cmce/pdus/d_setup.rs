use core::fmt;

use crate::cmce::enums::call_timeout::CallTimeout;
use crate::cmce::enums::transmission_grant::TransmissionGrant;
use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use crate::cmce::fields::basic_service_information::BasicServiceInformation;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-SETUP PDU (Clause 14.7.1.12).
/// This PDU shall be the call set-up message sent to the called MS.
/// Response expected: U-ALERT/U-CONNECT/-
/// Response to: -

// note 1: This information element is used by SS-PC, refer to ETSI EN 300 392-12-10 [15] and SS-PPC and ETSI EN 300 392-12-16 [16].
// note 2: For resolution of possible Facility (Talking Party Identifier)/Calling party identifier conflicts, refer to ETSI EN 300 392-12-3 [12], clause 5.2.1.5 and ETSI EN 300 392-12-1 [11], clause 4.3.5.
// note 3: Shall be conditional on the value of Calling Party Type Identifier (CPTI): • CPTI = 1 ⇒ Calling Party SSI; • CPTI = 2 ⇒ Calling Party SSI + Calling Party Extension.
#[derive(Debug)]
pub struct DSetup {
    /// Type1, 14 bits, Call identifier
    pub call_identifier: u16,
    /// Type1, 4 bits, Call time-out
    pub call_time_out: CallTimeout,
    /// Type1, 1 bits, Hook method selection
    pub hook_method_selection: bool,
    /// Type1, 1 bits, Simplex/duplex selection
    pub simplex_duplex_selection: bool,
    /// Type1, 8 bits, Basic service information
    pub basic_service_information: BasicServiceInformation,
    /// Type1, 2 bits, Transmission grant
    pub transmission_grant: TransmissionGrant,
    /// Type1, 1 bits, Transmission request permission
    /// Set to true to signal MSes they are allowed to send a U-TX DEMAND
    pub transmission_request_permission: bool,
    /// Type1, 4 bits, See note 1,
    pub call_priority: u8,
    /// Type2, 6 bits, Notification indicator
    pub notification_indicator: Option<u64>,
    /// Type2, 24 bits, Temporary address
    pub temporary_address: Option<u64>,
    /// Type2, 2 bits, See note 2,
    // pub calling_party_type_identifier: Option<u64>,
    /// Conditional 24 bits, See note 3, condition: calling_party_type_identifier == Some(1) || calling_party_type_identifier == Some(2)
    pub calling_party_address_ssi: Option<u32>,
    /// Conditional 24 bits, See note 3, condition: calling_party_type_identifier == Some(2)
    pub calling_party_extension: Option<u32>,
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
impl DSetup {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DSetup)?;

        // Type1
        let call_identifier = buffer.read_field(14, "call_identifier")? as u16;
        // Type1
        let val = buffer.read_field(4, "call_time_out")?;
        let call_time_out = CallTimeout::try_from(val).unwrap(); // Never fails

        // Type1
        let hook_method_selection = buffer.read_field(1, "hook_method_selection")? != 0;
        // Type1
        let simplex_duplex_selection = buffer.read_field(1, "simplex_duplex_selection")? != 0;
        // Type1
        let basic_service_information = BasicServiceInformation::from_bitbuf(buffer)?;
        // Type1
        let val = buffer.read_field(2, "transmission_grant")?;
        let transmission_grant = TransmissionGrant::try_from(val).unwrap(); // Never fails
        // Type1
        let transmission_request_permission = buffer.read_field(1, "transmission_request_permission")? != 0;
        // Type1
        let call_priority = buffer.read_field(4, "call_priority")? as u8;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let notification_indicator = typed::parse_type2_generic(obit, buffer, 6, "notification_indicator")?;
        // Type2
        let temporary_address = typed::parse_type2_generic(obit, buffer, 24, "temporary_address")?;
        // Type2
        let calling_party_type_identifier = typed::parse_type2_generic(obit, buffer, 2, "calling_party_type_identifier")?;
        // Conditional
        let calling_party_address_ssi = if obit && calling_party_type_identifier == Some(1) || calling_party_type_identifier == Some(2) {
            Some(buffer.read_field(24, "calling_party_address_ssi")? as u32)
        } else {
            None
        };
        // Conditional
        let calling_party_extension = if obit && calling_party_type_identifier == Some(2) {
            Some(buffer.read_field(24, "calling_party_extension")? as u32)
        } else {
            None
        };

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

        Ok(DSetup {
            call_identifier,
            call_time_out,
            hook_method_selection,
            simplex_duplex_selection,
            basic_service_information,
            transmission_grant,
            transmission_request_permission,
            call_priority,
            notification_indicator,
            temporary_address,
            // calling_party_type_identifier,
            calling_party_address_ssi,
            calling_party_extension,
            external_subscriber_number,
            facility,
            dm_ms_address,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DSetup.into_raw(), 5);
        // Type1
        buffer.write_bits(self.call_identifier as u64, 14);
        // Type1
        buffer.write_bits(self.call_time_out as u64, 4);
        // Type1
        buffer.write_bits(self.hook_method_selection as u64, 1);
        // Type1
        buffer.write_bits(self.simplex_duplex_selection as u64, 1);
        // Type1
        self.basic_service_information.to_bitbuf(buffer)?;
        // Type1
        buffer.write_bits(self.transmission_grant as u64, 2);
        // Type1
        buffer.write_bits(self.transmission_request_permission as u64, 1);
        // Type1
        buffer.write_bits(self.call_priority as u64, 4);

        // Check if any optional field present and place o-bit
        let obit = self.notification_indicator.is_some()
            || self.temporary_address.is_some()
            || self.calling_party_address_ssi.is_some()
            || self.calling_party_extension.is_some()
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
        typed::write_type2_generic(obit, buffer, self.temporary_address, 24);

        // Type2
        let calling_party_type_identifier = match (self.calling_party_address_ssi, self.calling_party_extension) {
            (None, None) => None,
            (Some(_), None) => Some(1),
            (Some(_), Some(_)) => Some(2),
            _ => {
                return Err(PduParseErr::InvalidValue {
                    field: "calling_party_type_identifier",
                    value: 0,
                });
            }
        };
        typed::write_type2_generic(obit, buffer, calling_party_type_identifier, 2);

        // Conditional
        if let Some(ref value) = self.calling_party_address_ssi {
            buffer.write_bits(*value as u64, 24);
        }
        // Conditional
        if let Some(ref value) = self.calling_party_extension {
            buffer.write_bits(*value as u64, 24);
        }
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

impl fmt::Display for DSetup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DSetup {{ call_identifier: {:?} call_time_out: {:?} hook_method_selection: {:?} simplex_duplex_selection: {:?} basic_service_information: {:?} transmission_grant: {:?} transmission_request_permission: {:?} call_priority: {:?} notification_indicator: {:?} temporary_address: {:?} calling_party_address_ssi: {:?} calling_party_extension: {:?} external_subscriber_number: {:?} facility: {:?} dm_ms_address: {:?} proprietary: {:?} }}",
            self.call_identifier,
            self.call_time_out,
            self.hook_method_selection,
            self.simplex_duplex_selection,
            self.basic_service_information,
            self.transmission_grant,
            self.transmission_request_permission,
            self.call_priority,
            self.notification_indicator,
            self.temporary_address,
            self.calling_party_address_ssi,
            self.calling_party_extension,
            self.external_subscriber_number,
            self.facility,
            self.dm_ms_address,
            self.proprietary,
        )
    }
}

#[cfg(test)]
mod tests {

    use tetra_core::debug;
    use tetra_saps::control::enums::{circuit_mode_type::CircuitModeType, communication_type::CommunicationType};

    use super::*;

    #[test]
    fn test_d_setup_lab() {
        debug::setup_logging_verbose();
        let mut buffer = BitBuffer::from_bitstr("00111000000000001000111000000010011000001001010000110111100010101100010");
        let pdu = DSetup::from_bitbuf(&mut buffer).unwrap();
        println!("Parsed DSetup: {:?}", pdu);
        assert!(buffer.get_len_remaining() == 0);

        assert_eq!(pdu.call_identifier, 4);
        assert_eq!(pdu.call_time_out, CallTimeout::T5m);
        assert_eq!(pdu.hook_method_selection, false);
        assert_eq!(pdu.simplex_duplex_selection, false);

        assert_eq!(pdu.basic_service_information.circuit_mode_type, CircuitModeType::TchS);
        assert_eq!(pdu.basic_service_information.encryption_flag, false);
        assert_eq!(pdu.basic_service_information.communication_type, CommunicationType::P2Mp);
        assert_eq!(pdu.basic_service_information.slots_per_frame, None);
        assert_eq!(pdu.basic_service_information.speech_service, Some(0));

        assert_eq!(pdu.transmission_grant, TransmissionGrant::GrantedToOtherUser);
        assert_eq!(pdu.transmission_request_permission, false);
        assert_eq!(pdu.call_priority, 0);
        assert!(pdu.notification_indicator.is_none());
        assert!(pdu.temporary_address.is_none());
        assert_eq!(pdu.calling_party_address_ssi.unwrap(), 910001);
        assert!(pdu.calling_party_extension.is_none());
        assert!(pdu.external_subscriber_number.is_none());
        assert!(pdu.facility.is_none());
        assert!(pdu.dm_ms_address.is_none());
        assert!(pdu.proprietary.is_none());

        let mut new = BitBuffer::new_autoexpand(71);
        pdu.to_bitbuf(&mut new).unwrap();
        assert_eq!(new.to_bitstr(), buffer.to_bitstr());
    }

    #[test]
    fn test_d_setup() {
        debug::setup_logging_verbose();
        let mut buffer = BitBuffer::from_bitstr("00111000000110000110000000000010011000001001010001111100100110001010000");
        let pdu = DSetup::from_bitbuf(&mut buffer).unwrap();
        println!("Parsed DSetup: {:?}", pdu);
        assert!(buffer.get_len_remaining() == 0);

        assert_eq!(pdu.call_identifier, 195);
        assert_eq!(pdu.call_time_out, CallTimeout::Infinite);
        assert_eq!(pdu.hook_method_selection, false);
        assert_eq!(pdu.simplex_duplex_selection, false);
        assert_eq!(pdu.basic_service_information.circuit_mode_type, CircuitModeType::TchS);
        assert_eq!(pdu.basic_service_information.encryption_flag, false);
        assert_eq!(pdu.basic_service_information.communication_type, CommunicationType::P2Mp);
        assert_eq!(pdu.basic_service_information.slots_per_frame, None);
        assert_eq!(pdu.basic_service_information.speech_service, Some(0));
        assert_eq!(pdu.transmission_grant, TransmissionGrant::GrantedToOtherUser);
        assert_eq!(pdu.transmission_request_permission, false);
        assert_eq!(pdu.call_priority, 0);
        assert!(pdu.notification_indicator.is_none());
        assert!(pdu.temporary_address.is_none());
        assert_eq!(pdu.calling_party_address_ssi.unwrap(), 2041384);
        assert!(pdu.calling_party_extension.is_none());
        assert!(pdu.external_subscriber_number.is_none());
        assert!(pdu.facility.is_none());
        assert!(pdu.dm_ms_address.is_none());
        assert!(pdu.proprietary.is_none());

        let mut new = BitBuffer::new_autoexpand(71);
        pdu.to_bitbuf(&mut new).unwrap();
        assert_eq!(new.to_bitstr(), buffer.to_bitstr());
    }
}
