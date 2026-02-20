use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_ul::CmcePduTypeUl, type3_elem_id::CmceType3ElemId};
use crate::cmce::fields::basic_service_information::BasicServiceInformation;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the U-SETUP PDU (Clause 14.7.2.10).
/// This PDU shall be the request for a call set-up from a MS.
/// Response expected: D-CALL PROCEEDING/D-ALERT/D-CONNECT
/// Response to: -

// note 1: This information element is used by SS-AS, refer to ETSI EN 300 392-12-8 [14].
// note 2: This information element is used by SS-PC, refer to ETSI EN 300 392-12-10 [15] and SS-PPC, refer to ETSI EN 300 392-12-16 [16].
// note 3: Refer to ETSI EN 300 392-12-1 [11].
// note 4: Shall be conditional on the value of Called Party Type Identifier (CPTI): CPTI = 0 → Called Party SNA (refer to ETS 300 392-12-7 [13]); CPTI = 1 → Called Party SSI; CPTI = 2 → Called Party SSI + Called Party Extension.
#[derive(Debug)]
pub struct USetup {
    /// Type1, 4 bits, See note 1. ETSI EN 300 392-12-8 Clause 5.2.2.3
    /// 0 = SS-AS not defined, 1-14 = SS-AS with selected area N, 15 = (usually) all areas
    pub area_selection: u8,
    /// Type1, 1 bits, Hook method selection
    /// 0 = No hook signalling (direct through-connect)
    /// 1 = Hook on/Hook off signalling
    pub hook_method_selection: bool,
    /// Type1, 1 bits, Simplex/duplex selection
    /// 0 = Simplex
    /// 1 = Duplex
    pub simplex_duplex_selection: bool,
    /// Type1, 8 bits, Basic service information
    pub basic_service_information: BasicServiceInformation,
    /// Type1, 1 bits, Request to transmit/send data
    /// The SwMI normally gives the first permission to transmit to the calling MS when a new call has been set-up. However,
    /// the calling user application may allow the called users to request permission to transmit first. The calling CC shall in
    /// that case set the "request to transmit" bit accordingly in the U-SETUP PDU.
    pub request_to_transmit_send_data: bool,
    /// Type1, 4 bits, See note 2,
    pub call_priority: u8,
    /// Type1, 2 bits, See note 3,
    pub clir_control: u8,
    /// Type1, 2 bits, Short/SSI/TSI,
    /// 0 = Short Number Address (SNA)
    /// 1 = Short Subscriber Identity (SSI)
    /// 2 = TETRA Subscriber Identity (TSI)
    /// 3 = Reserved
    pub called_party_type_identifier: u8,
    /// Conditional 8 bits, See note 4, condition: called_party_type_identifier == 0
    pub called_party_short_number_address: Option<u64>,
    /// Conditional 24 bits, See note 4, condition: called_party_type_identifier == 1 || called_party_type_identifier == 2
    pub called_party_ssi: Option<u64>,
    /// Conditional 24 bits, See note 4, condition: called_party_type_identifier == 2
    pub called_party_extension: Option<u64>,
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
impl USetup {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeUl::USetup)?;

        // Type1
        let area_selection = buffer.read_field(4, "area_selection")? as u8;
        // Type1
        let hook_method_selection = buffer.read_field(1, "hook_method_selection")? != 0;
        // Type1
        let simplex_duplex_selection = buffer.read_field(1, "simplex_duplex_selection")? != 0;
        // Type1
        let basic_service_information = BasicServiceInformation::from_bitbuf(buffer)?;
        // Type1
        let request_to_transmit_send_data = buffer.read_field(1, "request_to_transmit_send_data")? != 0;
        // Type1
        let call_priority = buffer.read_field(4, "call_priority")? as u8;
        // Type1
        let clir_control = buffer.read_field(2, "clir_control")? as u8;
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

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

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

        Ok(USetup {
            area_selection,
            hook_method_selection,
            simplex_duplex_selection,
            basic_service_information,
            request_to_transmit_send_data,
            call_priority,
            clir_control,
            called_party_type_identifier,
            called_party_short_number_address,
            called_party_ssi,
            called_party_extension,
            external_subscriber_number,
            facility,
            dm_ms_address,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeUl::USetup.into_raw(), 5);
        // Type1
        buffer.write_bits(self.area_selection as u64, 4);
        // Type1
        buffer.write_bits(self.hook_method_selection as u64, 1);
        // Type1
        buffer.write_bits(self.simplex_duplex_selection as u64, 1);
        // Type1
        self.basic_service_information.to_bitbuf(buffer)?;
        // Type1
        buffer.write_bits(self.request_to_transmit_send_data as u64, 1);
        // Type1
        buffer.write_bits(self.call_priority as u64, 4);
        // Type1
        buffer.write_bits(self.clir_control as u64, 2);
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

        // Check if any optional field present and place o-bit
        let obit = self.external_subscriber_number.is_some()
            || self.facility.is_some()
            || self.dm_ms_address.is_some()
            || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
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

impl fmt::Display for USetup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "USetup {{ area_selection: {:?} hook_method_selection: {:?} simplex_duplex_selection: {:?} basic_service_information: {:?} request_to_transmit_send_data: {:?} call_priority: {:?} clir_control: {:?} called_party_type_identifier: {:?} called_party_short_number_address: {:?} called_party_ssi: {:?} called_party_extension: {:?} external_subscriber_number: {:?} facility: {:?} dm_ms_address: {:?} proprietary: {:?} }}",
            self.area_selection,
            self.hook_method_selection,
            self.simplex_duplex_selection,
            self.basic_service_information,
            self.request_to_transmit_send_data,
            self.call_priority,
            self.clir_control,
            self.called_party_type_identifier,
            self.called_party_short_number_address,
            self.called_party_ssi,
            self.called_party_extension,
            self.external_subscriber_number,
            self.facility,
            self.dm_ms_address,
            self.proprietary,
        )
    }
}
