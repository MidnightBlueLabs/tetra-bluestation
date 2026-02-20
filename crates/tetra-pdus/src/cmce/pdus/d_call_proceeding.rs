use core::fmt;

use crate::cmce::enums::call_status::CallStatus;
use crate::cmce::enums::call_timeout_setup_phase::CallTimeoutSetupPhase;
use crate::cmce::enums::{cmce_pdu_type_dl::CmcePduTypeDl, type3_elem_id::CmceType3ElemId};
use crate::cmce::fields::basic_service_information::BasicServiceInformation;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, expect_pdu_type, pdu_parse_error::PduParseErr};

/// Representation of the D-CALL PROCEEDING PDU (Clause 14.7.1.2).
/// This PDU shall be the acknowledgement from the infrastructure to call set-up request indicating that the call is proceeding.
/// Response expected: -
/// Response to: U-SETUP

// note 1: If different from requested.
#[derive(Debug)]
pub struct DCallProceeding {
    /// Type1, 14 bits, Call identifier
    pub call_identifier: u16,
    /// Type1, 3 bits, Call time-out, set-up phase
    pub call_time_out_set_up_phase: CallTimeoutSetupPhase,
    /// Type1, 1 bits, Hook method selection
    pub hook_method_selection: bool,
    /// Type1, 1 bits, Simplex/duplex selection
    pub simplex_duplex_selection: bool,
    /// Type2, 8 bits, If different from requested.,
    pub basic_service_information: Option<BasicServiceInformation>,
    /// Type2, 3 bits, Call status
    pub call_status: Option<CallStatus>,
    /// Type2, 6 bits, Notification indicator
    pub notification_indicator: Option<u64>,
    /// Type3, Facility
    pub facility: Option<Type3FieldGeneric>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DCallProceeding {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(5, "pdu_type")?;
        expect_pdu_type!(pdu_type, CmcePduTypeDl::DCallProceeding)?;

        // Type1
        let call_identifier = buffer.read_field(14, "call_identifier")? as u16;
        // Type1
        let val = buffer.read_field(3, "call_time_out_set_up_phase")?;
        let call_time_out_set_up_phase = CallTimeoutSetupPhase::try_from(val).unwrap(); // Never fails

        // Type1
        let hook_method_selection = buffer.read_field(1, "hook_method_selection")? != 0;
        // Type1
        let simplex_duplex_selection = buffer.read_field(1, "simplex_duplex_selection")? != 0;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let basic_service_information = typed::parse_type2_struct(obit, buffer, BasicServiceInformation::from_bitbuf)?;
        // Type2
        let val = typed::parse_type2_generic(obit, buffer, 3, "call_status")?;
        let call_status = match val {
            None => None,
            Some(val) => Some(CallStatus::try_from(val).map_err(|_| PduParseErr::InvalidValue {
                field: "call_status",
                value: val,
            })?),
        };

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

        Ok(DCallProceeding {
            call_identifier,
            call_time_out_set_up_phase,
            hook_method_selection,
            simplex_duplex_selection,
            basic_service_information,
            call_status,
            notification_indicator,
            facility,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(CmcePduTypeDl::DCallProceeding.into_raw(), 5);
        // Type1
        buffer.write_bits(self.call_identifier as u64, 14);
        // Type1
        buffer.write_bits(self.call_time_out_set_up_phase as u64, 3);
        // Type1
        buffer.write_bits(self.hook_method_selection as u64, 1);
        // Type1
        buffer.write_bits(self.simplex_duplex_selection as u64, 1);

        // Check if any optional field present and place o-bit
        let obit = self.basic_service_information.is_some()
            || self.call_status.is_some()
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
        typed::write_type2_generic(obit, buffer, self.call_status.map(|x| x.into()), 3);

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

impl fmt::Display for DCallProceeding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DCallProceeding {{ call_identifier: {:?} call_time_out_set_up_phase: {:?} hook_method_selection: {:?} simplex_duplex_selection: {:?} basic_service_information: {:?} call_status: {:?} notification_indicator: {:?} facility: {:?} proprietary: {:?} }}",
            self.call_identifier,
            self.call_time_out_set_up_phase,
            self.hook_method_selection,
            self.simplex_duplex_selection,
            self.basic_service_information,
            self.call_status,
            self.notification_indicator,
            self.facility,
            self.proprietary,
        )
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_parse_d_call_proceeding() {
        debug::setup_logging_verbose();
        let test_vec = "0000100000000000100110000";
        let mut buffer = BitBuffer::from_bitstr(test_vec);
        let pdu = DCallProceeding::from_bitbuf(&mut buffer).unwrap();
        println!("Parsed DCallProceeding: {:?}", pdu);

        assert_eq!(pdu.call_identifier, 4);
        assert_eq!(pdu.call_time_out_set_up_phase, CallTimeoutSetupPhase::T30s);
        assert_eq!(pdu.hook_method_selection, false);
        assert_eq!(pdu.simplex_duplex_selection, false);
        // assert_eq!(pdu.basic_service_information, None);
        assert_eq!(pdu.call_status, None);
        assert_eq!(pdu.notification_indicator, None);
        assert_eq!(pdu.facility, None);
        assert_eq!(pdu.proprietary, None);

        assert!(buffer.get_len_remaining() == 0);
    }

    #[test]
    fn test_parse_d_call_proceeding_with_service_information() {
        debug::setup_logging_verbose();
        let test_vec = "0000100000000000100110001100000100000"; // 0000000
        let mut buffer = BitBuffer::from_bitstr(test_vec);
        let pdu = DCallProceeding::from_bitbuf(&mut buffer).unwrap();
        println!("Parsed DCallProceeding: {:?}", pdu);

        assert_eq!(pdu.call_identifier, 4);
        assert_eq!(pdu.call_time_out_set_up_phase, CallTimeoutSetupPhase::T30s);
        assert_eq!(pdu.hook_method_selection, false);
        assert_eq!(pdu.simplex_duplex_selection, false);
        // assert_eq!(pdu.basic_service_information, None);
        assert_eq!(pdu.call_status, None);
        assert_eq!(pdu.notification_indicator, None);
        assert_eq!(pdu.facility, None);
        assert_eq!(pdu.proprietary, None);

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);
        assert!(buffer.get_len_remaining() == 0);
    }
}
