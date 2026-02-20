use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::location_update_type::LocationUpdateType;
use crate::mm::enums::mm_pdu_type_dl::MmPduTypeDl;
use crate::mm::enums::type34_elem_id_dl::MmType34ElemIdDl;
use crate::mm::fields::energy_saving_information::EnergySavingInformation;
use crate::mm::fields::group_identity_location_accept::GroupIdentityLocationAccept;

/// Representation of the D-LOCATION UPDATE ACCEPT PDU (Clause 16.9.2.7).
/// The infrastructure sends this message to the MS to indicate that updating in the network has been completed.
/// Response expected: -
/// Response to: U-LOCATION UPDATE DEMAND

// Note: The MS shall accept the type 3/4 information elements both in the numerical order as described in annex E and in the order shown in this table.
#[derive(Debug)]
pub struct DLocationUpdateAccept {
    /// Type1, 3 bits, Location update accept type
    pub location_update_accept_type: LocationUpdateType,
    /// Type2, 24 bits, ASSI/(V)ASSI of MS,
    pub ssi: Option<u64>,
    /// Type2, 24 bits, MNI of MS,
    pub address_extension: Option<u64>,
    /// Type2, 16 bits, Subscriber class
    pub subscriber_class: Option<u64>,
    /// Type2, 14 bits, Energy saving information
    pub energy_saving_information: Option<EnergySavingInformation>,
    /// Type2, 6 bits, SCCH information and distribution on 18th frame
    pub scch_information_and_distribution_on_18th_frame: Option<u64>,
    /// Type4, See note,
    pub new_registered_area: Option<Type4FieldGeneric>,
    /// Type3, See ETSI EN 300 392-7 [8],
    pub security_downlink: Option<Type3FieldGeneric>,
    /// Type3, See note,
    pub group_identity_location_accept: Option<GroupIdentityLocationAccept>,
    /// Type3, See note,
    pub default_group_attachment_lifetime: Option<Type3FieldGeneric>,
    /// Type3, See ETSI EN 300 392-7 [8],
    pub authentication_downlink: Option<Type3FieldGeneric>,
    /// Type4, See ETSI EN 300 392-7 [8],
    pub group_identity_security_related_information: Option<Type4FieldGeneric>,
    /// Type3, Cell type control
    pub cell_type_control: Option<Type3FieldGeneric>,
    /// Type3, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl DLocationUpdateAccept {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeDl::DLocationUpdateAccept)?;

        // Type1
        let val: u64 = buffer.read_field(3, "location_update_accept_type")?;
        let result = LocationUpdateType::try_from(val);
        let location_update_accept_type = match result {
            Ok(x) => x,
            Err(_) => {
                return Err(PduParseErr::InvalidValue {
                    field: "location_update_accept_type",
                    value: val,
                });
            }
        };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let ssi = typed::parse_type2_generic(obit, buffer, 24, "ssi")?;
        // Type2
        let address_extension = typed::parse_type2_generic(obit, buffer, 24, "address_extension")?;
        // Type2
        let subscriber_class = typed::parse_type2_generic(obit, buffer, 16, "subscriber_class")?;
        // Type2
        let energy_saving_information = typed::parse_type2_struct(obit, buffer, EnergySavingInformation::from_bitbuf)?;
        // Type2
        let scch_information_and_distribution_on_18th_frame =
            typed::parse_type2_generic(obit, buffer, 6, "scch_information_and_distribution_on_18th_frame")?;

        // Type4
        let new_registered_area = typed::parse_type4_generic(obit, buffer, MmType34ElemIdDl::NewRegisteredArea)?;

        // Type3
        let security_downlink = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::SecurityDownlink)?;

        // Type3
        let group_identity_location_accept = typed::parse_type3_struct(
            obit,
            buffer,
            MmType34ElemIdDl::GroupIdentityLocationAccept,
            GroupIdentityLocationAccept::from_bitbuf,
        )?;

        // Type3
        let default_group_attachment_lifetime = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::DefaultGroupAttachLifetime)?;

        // Type3
        let authentication_downlink = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::AuthenticationDownlink)?;

        // Type4
        let group_identity_security_related_information =
            typed::parse_type4_generic(obit, buffer, MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation)?;

        // Type3
        let cell_type_control = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::CellTypeControl)?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, MmType34ElemIdDl::Proprietary)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(DLocationUpdateAccept {
            location_update_accept_type,
            ssi,
            address_extension,
            subscriber_class,
            energy_saving_information,
            scch_information_and_distribution_on_18th_frame,
            new_registered_area,
            security_downlink,
            group_identity_location_accept,
            default_group_attachment_lifetime,
            authentication_downlink,
            group_identity_security_related_information,
            cell_type_control,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeDl::DLocationUpdateAccept.into_raw(), 4);
        // Type1
        buffer.write_bits(self.location_update_accept_type as u64, 3);

        // Check if any optional field present and place o-bit
        let obit = self.ssi.is_some()
            || self.address_extension.is_some()
            || self.subscriber_class.is_some()
            || self.energy_saving_information.is_some()
            || self.scch_information_and_distribution_on_18th_frame.is_some()
            || self.new_registered_area.is_some()
            || self.security_downlink.is_some()
            || self.group_identity_location_accept.is_some()
            || self.default_group_attachment_lifetime.is_some()
            || self.authentication_downlink.is_some()
            || self.group_identity_security_related_information.is_some()
            || self.cell_type_control.is_some()
            || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.ssi, 24);

        // Type2
        typed::write_type2_generic(obit, buffer, self.address_extension, 24);

        // Type2
        typed::write_type2_generic(obit, buffer, self.subscriber_class, 16);

        // Type2
        typed::write_type2_struct(obit, buffer, &self.energy_saving_information, EnergySavingInformation::to_bitbuf)?;

        // Type2
        typed::write_type2_generic(obit, buffer, self.scch_information_and_distribution_on_18th_frame, 6);

        // Type4
        typed::write_type4_todo(obit, buffer, &self.new_registered_area, MmType34ElemIdDl::NewRegisteredArea)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.security_downlink, MmType34ElemIdDl::SecurityDownlink)?;

        // Type3
        typed::write_type3_struct(
            obit,
            buffer,
            &self.group_identity_location_accept,
            MmType34ElemIdDl::GroupIdentityLocationAccept,
            GroupIdentityLocationAccept::to_bitbuf,
        )?;

        // Type3
        typed::write_type3_generic(
            obit,
            buffer,
            &self.default_group_attachment_lifetime,
            MmType34ElemIdDl::DefaultGroupAttachLifetime,
        )?;

        // Type3
        typed::write_type3_generic(
            obit,
            buffer,
            &self.authentication_downlink,
            MmType34ElemIdDl::AuthenticationDownlink,
        )?;

        // Type4
        typed::write_type4_todo(
            obit,
            buffer,
            &self.group_identity_security_related_information,
            MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation,
        )?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.cell_type_control, MmType34ElemIdDl::CellTypeControl)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.proprietary, MmType34ElemIdDl::Proprietary)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);

        Ok(())
    }
}

impl fmt::Display for DLocationUpdateAccept {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "DLocationUpdateAccept {{ location_update_accept_type: {:?} ssi: {:?} address_extension: {:?} subscriber_class: {:?} energy_saving_information: {:?} scch_information_and_distribution_on_18th_frame: {:?} new_registered_area: {:?} security_downlink: {:?} group_identity_location_accept: {:?} default_group_attachment_lifetime: {:?} authentication_downlink: {:?} group_identity_security_related_information: {:?} cell_type_control: {:?} proprietary: {:?} }}",
            self.location_update_accept_type,
            self.ssi,
            self.address_extension,
            self.subscriber_class,
            self.energy_saving_information,
            self.scch_information_and_distribution_on_18th_frame,
            self.new_registered_area,
            self.security_downlink,
            self.group_identity_location_accept,
            self.default_group_attachment_lifetime,
            self.authentication_downlink,
            self.group_identity_security_related_information,
            self.cell_type_control,
            self.proprietary,
        )
    }
}

#[cfg(test)]
mod tests {
    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_d_location_update_accept_with_group_identity_attachment() {
        // Self-generated vector, seems to be properly accepted
        debug::setup_logging_verbose();
        let test_vec = "0101011110001111100100011111011100000101010000011101000110111000001001100000010111000000000000000000000001101000";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = DLocationUpdateAccept::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        assert!(buf_in.get_len_remaining() == 0, "Buffer not fully consumed");

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);
    }
}
