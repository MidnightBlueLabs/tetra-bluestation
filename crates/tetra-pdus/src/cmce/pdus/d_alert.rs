use core::fmt;

use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use crate::cmce::fields::basic_service_information::BasicServiceInformation;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-ALERT PDU (Clause 14.7.1.1).
/// This PDU shall be an information to the originating MS that the call is proceeding and the connecting party has been alerted.
/// Response expected: -
/// Response to: U-SETUP

// note 1: This information element is not used in this edition of the present document and its value shall be set to "1" (equivalent to "Hook on/Hook off signalling" for backwards compatibility with edition 1 of the present document – refer to Table 14.62).
// note 2: If different from requested.
#[derive(Debug)]
pub struct DAlert {
    /// Type1, 14 bits, Call identifier
    pub call_identifier: u16,
    /// Type1, 3 bits, Call time-out, set-up phase
    pub call_time_out_set_up_phase: u8,
    /// Type1, 1 bits, See note 1,
    pub reserved: bool,
    /// Type1, 1 bits, Simplex/duplex selection
    pub simplex_duplex_selection: bool,
    /// Type1, 1 bits, Call queued
    pub call_queued: bool,
    /// Type2, 8 bits, See note 2,
    pub basic_service_information: Option<BasicServiceInformation>,
    /// Type2, 6 bits, Notification indicator
    pub notification_indicator: Option<u64>,
    /// Type3, Facility
    pub facility: Option<Type3FieldGeneric>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DAlert {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DAlert)?;

        // Type1
        let call_identifier = buffer.read_field(14, "call_identifier")? as u16;
        // Type1
        let call_time_out_set_up_phase = buffer.read_field(3, "call_time_out_set_up_phase")? as u8;
        // Type1
        let reserved = buffer.read_field(1, "reserved")? != 0;
        // Type1
        let simplex_duplex_selection = buffer.read_field(1, "simplex_duplex_selection")? != 0;
        // Type1
        let call_queued = buffer.read_field(1, "call_queued")? != 0;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let basic_service_information = typed::parse_type2_struct(obit, buffer, BasicServiceInformation::from_bitbuf)?;

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

        Ok(DAlert {
            call_identifier,
            call_time_out_set_up_phase,
            reserved,
            simplex_duplex_selection,
            call_queued,
            basic_service_information,
            notification_indicator,
            facility,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DAlert.into_raw(), 5);
        // Type1
        buffer.write_bits(self.call_identifier as u64, 14);
        // Type1
        buffer.write_bits(self.call_time_out_set_up_phase as u64, 3);
        // Type1
        buffer.write_bits(self.reserved as u64, 1);
        // Type1
        buffer.write_bits(self.simplex_duplex_selection as u64, 1);
        // Type1
        buffer.write_bits(self.call_queued as u64, 1);

        // Check if any optional field present and place o-bit
        let obit = self.basic_service_information.is_some()
            || self.notification_indicator.is_some()
            || self.facility.is_some()
            || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_struct(obit, buffer, &self.basic_service_information, BasicServiceInformation::to_bitbuf)?;

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

impl fmt::Display for DAlert {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DAlert {{ call_identifier: {:?} call_time_out_set_up_phase: {:?} reserved: {:?} simplex_duplex_selection: {:?} call_queued: {:?} basic_service_information: {:?} notification_indicator: {:?} facility: {:?} proprietary: {:?} }}",
            self.call_identifier,
            self.call_time_out_set_up_phase,
            self.reserved,
            self.simplex_duplex_selection,
            self.call_queued,
            self.basic_service_information,
            self.notification_indicator,
            self.facility,
            self.proprietary,
        )
    }
}
