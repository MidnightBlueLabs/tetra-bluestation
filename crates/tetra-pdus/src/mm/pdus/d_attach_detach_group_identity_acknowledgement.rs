use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;
use crate::mm::enums::type34_elem_id_dl::MmType34ElemIdDl;
use crate::mm::fields::group_identity_downlink::GroupIdentityDownlink;

/// Representation of the D-ATTACH/DETACH GROUP IDENTITY ACKNOWLEDGEMENT PDU (Clause 16.9.2.2).
/// The infrastructure sends this message to the MS to acknowledge MS-initiated attachment/detachment of group identities.
/// Response expected: -
/// Response to: U-ATTACH/DETACH GROUP IDENTITY

// Note: The MS shall accept the type 3/4 information elements both in the numerical order as described in annex E and in the order shown in this table.
#[derive(Debug)]
pub struct DAttachDetachGroupIdentityAcknowledgement {
    /// Type1, 1 bits, Group identity accept/reject
    pub group_identity_accept_reject: u8,
    /// Type1, 1 bits, Reserved
    pub reserved: bool,
    /// Type3, See note,
    pub proprietary: Option<Type3FieldGeneric>,
    /// Type4, See note,
    pub group_identity_downlink: Option<Vec<GroupIdentityDownlink>>,
    /// Type4, See ETSI EN 300 392-7 [8] and note,
    pub group_identity_security_related_information: Option<Type4FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DAttachDetachGroupIdentityAcknowledgement {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::DAttachDetachGroupIdentityAcknowledgement)?;

        // Type1
        let group_identity_accept_reject = buffer.read_field(1, "group_identity_accept_reject")? as u8;
        // Type1
        let reserved = buffer.read_field(1, "reserved")? != 0;

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::Proprietary)?;

        // Type4
        let group_identity_downlink = typed::parse_type4_struct(
            obit,
            buffer,
            MmType34ElemIdDl::GroupIdentityDownlink,
            GroupIdentityDownlink::from_bitbuf,
        )?;

        // Type4
        let group_identity_security_related_information =
            typed::parse_type4_generic(obit, buffer, MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DAttachDetachGroupIdentityAcknowledgement {
            group_identity_accept_reject,
            reserved,
            proprietary,
            group_identity_downlink,
            group_identity_security_related_information,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeDl::DAttachDetachGroupIdentityAcknowledgement.into_raw(), 4);
        // Type1
        buffer.write_bits(self.group_identity_accept_reject as u64, 1);
        // Type1
        buffer.write_bits(self.reserved as u64, 1);

        // Check if any optional field present and place o-bit
        let obit = self.proprietary.is_some()
            || self.group_identity_downlink.is_some()
            || self.group_identity_security_related_information.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type3
        typed::write_type3_generic(obit, buffer, &self.proprietary, MmType34ElemIdDl::Proprietary)?;

        // Type4
        typed::write_type4_struct(
            obit,
            buffer,
            &self.group_identity_downlink,
            MmType34ElemIdDl::GroupIdentityDownlink,
            GroupIdentityDownlink::to_bitbuf,
        )?;

        // Type4
        typed::write_type4_todo(
            obit,
            buffer,
            &self.group_identity_security_related_information,
            MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation,
        )?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for DAttachDetachGroupIdentityAcknowledgement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DAttachDetachGroupIdentityAcknowledgement {{ group_identity_accept_reject: {:?} reserved: {:?} proprietary: {:?} group_identity_downlink: {:?} group_identity_security_related_information: {:?} }}",
            self.group_identity_accept_reject,
            self.reserved,
            self.proprietary,
            self.group_identity_downlink,
            self.group_identity_security_related_information,
        )
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_d_attach_detach_group_identity_ack() {
        // 10110011011100000100110000001011100000000110101000110011100000
        // |--|         identifier
        //     |        accept/reject
        //      |       reserved
        //       ||                                                         obit, mbit
        //         |--|                                                     identifier: 0x7 GroupIdentityDownlink
        //             |---------|                                          len: 38
        //                        |------------------------------------|    field
        //                                                              |   closing mbit
        //
        // 000001 01110010000001010100011001110000
        // |----|           num elems: 1
        //        |         attach/detach type identifier
        //         ||       fetime: until next location update
        //           |-|    class of usage: 4
        //              ||  type identifier
        //                |----------------------| gssi: 0x000000

        // Vec from lab
        debug::setup_logging_verbose();
        let test_vec = "10110011011100000100110000001011100000000110101000110011100000";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = DAttachDetachGroupIdentityAcknowledgement::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        assert!(buf_in.get_len_remaining() == 0, "Buffer not fully consumed");

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);
    }
}
