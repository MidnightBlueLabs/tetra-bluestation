use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_ul::MmPduTypeUl;
use crate::mm::enums::type34_elem_id_ul::MmType34ElemIdUl;
use crate::mm::fields::group_identity_uplink::GroupIdentityUplink;

/// Representation of the U-ATTACH/DETACH GROUP IDENTITY PDU (Clause 16.9.3.1).
/// The MS sends this message to the infrastructure to indicate attachment/detachment of group identities in the MS or to initiate a group report request or give a group report response.
/// Response expected: D-ATTACH/DETACH GROUP IDENTITY ACKNOWLEDGEMENT
/// Response to: -/D-ATTACH/DETACH GROUP IDENTITY (report request)

#[derive(Debug)]
pub struct UAttachDetachGroupIdentity {
    /// Type1, 1 bits, Group identity report
    pub group_identity_report: bool,
    /// Type1, 1 bits, Group identity attach/detach mode. 0 = amendment, 1 = detach all and attach to specified groups
    pub group_identity_attach_detach_mode: bool,
    /// Type3, Group report response
    pub group_report_response: Option<Type3FieldGeneric>,
    /// Type4, Group identity uplink
    pub group_identity_uplink: Option<Vec<GroupIdentityUplink>>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl UAttachDetachGroupIdentity {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeUl::UAttachDetachGroupIdentity)?;

        // Type1
        let group_identity_report = buffer.read_field(1, "group_identity_report")? != 0;
        // Type1
        let group_identity_attach_detach_mode = buffer.read_field(1, "group_identity_attach_detach_mode")? != 0;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type3 - stores raw data, so use existing approach
        let group_report_response = typed::parse_type3_generic(obit, buffer, MmType34ElemIdUl::GroupReportResponse)?;

        // Type4 - parses to structs, use generic helper
        let group_identity_uplink = typed::parse_type4_struct(
            obit,
            buffer,
            MmType34ElemIdUl::GroupIdentityUplink,
            GroupIdentityUplink::from_bitbuf,
        )?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, MmType34ElemIdUl::Proprietary)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(UAttachDetachGroupIdentity {
            group_identity_report,
            group_identity_attach_detach_mode,
            group_report_response,
            group_identity_uplink,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeUl::UAttachDetachGroupIdentity.into_raw(), 4);
        // Type1
        buffer.write_bits(self.group_identity_report as u64, 1);
        // Type1
        buffer.write_bits(self.group_identity_attach_detach_mode as u64, 1);

        // Check if any optional field present and place o-bit
        let obit = self.group_report_response.is_some() || self.group_identity_uplink.is_some() || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type3
        typed::write_type3_generic(obit, buffer, &self.group_report_response, MmType34ElemIdUl::GroupReportResponse)?;

        // Type4
        typed::write_type4_struct(
            obit,
            buffer,
            &self.group_identity_uplink,
            MmType34ElemIdUl::GroupIdentityUplink,
            GroupIdentityUplink::to_bitbuf,
        )?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.proprietary, MmType34ElemIdUl::Proprietary)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for UAttachDetachGroupIdentity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "UAttachDetachGroupIdentity {{ group_identity_report: {:?} group_identity_attach_detach_mode: {:?} group_report_response: {:?} group_identity_uplink: {:?} proprietary: {:?} }}",
            self.group_identity_report,
            self.group_identity_attach_detach_mode,
            self.group_report_response,
            self.group_identity_uplink,
            self.proprietary,
        )
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_u_attach_detach_group_identity() {
        // 0111 0 1 1 11000000001001000000010100000000110101000110011100000
        // |--| PDU type
        //      | | group identity report = 0, group identity attach/detach mode = 1 (reset all prev and reattach to specified groups)
        //          | obit: fields follow
        //            | mbit:  group report response is present
        //             |--| field_id = 8 GroupIdentityUplink
        //                 |---------| len = 000 0010 0100 0x24 = 36
        //                            |----------------------------------| 	field contents
        //                                                                | trailing mbit

        // Vector from Sepura SC20
        debug::setup_logging_verbose();
        let test_vec = "011101111000000001001000000010100000000110101000110011100000";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = UAttachDetachGroupIdentity::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);
    }
}
