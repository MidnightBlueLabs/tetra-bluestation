use core::fmt;

use crate::cmce::enums::call_timeout::CallTimeout;
use crate::cmce::enums::transmission_grant::TransmissionGrant;
use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use crate::cmce::fields::basic_service_information::BasicServiceInformation;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-CONNECT PDU (Clause 14.7.1.4).
/// This PDU shall be the order to the calling MS to through-connect.
/// Response expected: None
/// Response to: U-SETUP

// note 1: Basic service information element: If different from requested.
#[derive(Debug)]
pub struct DConnect {
    /// Type1, 14 bits, Call identifier
    pub call_identifier: u16,
    /// Type1, 4 bits, Call time-out
    pub call_time_out: CallTimeout,
    /// Type1, 1 bits, Hook method selection
    pub hook_method_selection: bool,
    /// Type1, 1 bits, Simplex/duplex selection
    pub simplex_duplex_selection: bool,
    /// Type1, 2 bits, Transmission grant
    pub transmission_grant: TransmissionGrant,
    /// Type1, 1 bits, Transmission request permission
    /// Set to true to signal MSes they are allowed to send a U-TX DEMAND
    pub transmission_request_permission: bool,
    /// Type1, 1 bits, Call ownership
    pub call_ownership: bool,
    /// Type2, 4 bits, Call priority
    pub call_priority: Option<u64>,
    /// Type2, 8 bits, See note,
    pub basic_service_information: Option<BasicServiceInformation>,
    /// Type2, 24 bits, Temporary address
    pub temporary_address: Option<u64>,
    /// Type2, 6 bits, Notification indicator
    pub notification_indicator: Option<u64>,
    /// Type3, Facility
    pub facility: Option<Type3FieldGeneric>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DConnect {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DConnect)?;

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
        let val = buffer.read_field(2, "transmission_grant")?;
        let transmission_grant = TransmissionGrant::try_from(val).unwrap(); // Never fails

        // Type1
        let transmission_request_permission = buffer.read_field(1, "transmission_request_permission")? != 0;
        // Type1
        let call_ownership = buffer.read_field(1, "call_ownership")? != 0;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let call_priority = typed::parse_type2_generic(obit, buffer, 4, "call_priority")?;
        // Type2
        let basic_service_information = typed::parse_type2_struct(obit, buffer, BasicServiceInformation::from_bitbuf)?;
        // Type2
        let temporary_address = typed::parse_type2_generic(obit, buffer, 24, "temporary_address")?;
        // Type2
        let notification_indicator = typed::parse_type2_generic(obit, buffer, 6, "notification_indicator")?;

        // Type3
        let facility = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::Facility)?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, CmceType3ElemId::Proprietary)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DConnect {
            call_identifier,
            call_time_out,
            hook_method_selection,
            simplex_duplex_selection,
            transmission_grant,
            transmission_request_permission,
            call_ownership,
            call_priority,
            basic_service_information,
            temporary_address,
            notification_indicator,
            facility,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DConnect.into_raw(), 5);
        // Type1
        buffer.write_bits(self.call_identifier as u64, 14);
        // Type1
        buffer.write_bits(self.call_time_out as u64, 4);
        // Type1
        buffer.write_bits(self.hook_method_selection as u64, 1);
        // Type1
        buffer.write_bits(self.simplex_duplex_selection as u64, 1);
        // Type1
        buffer.write_bits(self.transmission_grant as u64, 2);
        // Type1
        buffer.write_bits(self.transmission_request_permission as u64, 1);
        // Type1
        buffer.write_bits(self.call_ownership as u64, 1);

        // Check if any optional field present and place o-bit
        let obit = self.call_priority.is_some()
            || self.basic_service_information.is_some()
            || self.temporary_address.is_some()
            || self.notification_indicator.is_some()
            || self.facility.is_some()
            || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.call_priority, 4);

        // Type2
        typed::write_type2_struct(obit, buffer, &self.basic_service_information, BasicServiceInformation::to_bitbuf)?;

        // Type2
        typed::write_type2_generic(obit, buffer, self.temporary_address, 24);

        // Type2
        typed::write_type2_generic(obit, buffer, self.notification_indicator, 6);

        // Type3
        typed::write_type3_generic(obit, buffer, &self.facility, CmceType3ElemId::Facility)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.proprietary, CmceType3ElemId::Proprietary)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DConnect {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DConnect {{ call_identifier: {:?} call_time_out: {:?} hook_method_selection: {:?} simplex_duplex_selection: {:?} transmission_grant: {:?} transmission_request_permission: {:?} call_ownership: {:?} call_priority: {:?} basic_service_information: {:?} temporary_address: {:?} notification_indicator: {:?} facility: {:?} proprietary: {:?} }}",
            self.call_identifier,
            self.call_time_out,
            self.hook_method_selection,
            self.simplex_duplex_selection,
            self.transmission_grant,
            self.transmission_request_permission,
            self.call_ownership,
            self.call_priority,
            self.basic_service_information,
            self.temporary_address,
            self.notification_indicator,
            self.facility,
            self.proprietary,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_d_connect() {
        let mut buffer = BitBuffer::from_bitstr("000100000000000010001110000000");
        let result = DConnect::from_bitbuf(&mut buffer);
        println!("Parsed DConnect: {:?}", result);

        assert!(result.is_ok());
        let d_connect = result.unwrap();
        assert_eq!(d_connect.call_identifier, 4);
        assert_eq!(d_connect.call_time_out, CallTimeout::T5m);
        assert_eq!(d_connect.hook_method_selection, false);
        assert_eq!(d_connect.simplex_duplex_selection, false);
        assert_eq!(d_connect.transmission_grant, TransmissionGrant::Granted);
        assert_eq!(d_connect.transmission_request_permission, false);
        assert_eq!(d_connect.call_ownership, false);
        assert!(d_connect.call_priority.is_none());
        assert!(d_connect.basic_service_information.is_none());
        assert!(d_connect.temporary_address.is_none());
        assert!(d_connect.notification_indicator.is_none());
        assert!(d_connect.facility.is_none());
        assert!(d_connect.proprietary.is_none());
    }
}
