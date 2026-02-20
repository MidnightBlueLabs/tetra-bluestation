use core::fmt;

use tetra_core::expect_pdu_type;
use tetra_core::typed_pdu_fields::*;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::enums::energy_saving_mode::EnergySavingMode;
use crate::mm::enums::location_update_type::LocationUpdateType;
use crate::mm::enums::mm_pdu_type_ul::MmPduTypeUl;
use crate::mm::enums::type34_elem_id_ul::MmType34ElemIdUl;
use crate::mm::fields::group_identity_location_demand::GroupIdentityLocationDemand;

/// Representation of the U-LOCATION UPDATE DEMAND PDU (Clause 16.9.3.4).
/// The MS sends this message to the infrastructure to request update of its location registration.
/// Response expected: D-LOCATION UPDATE ACCEPT/D-LOCATION UPDATE REJECT
/// Response to: -/D-LOCATION UPDATE COMMAND

// note 1: Information element "Ciphering parameters" is not present if "Cipher control" is set to "0" (ciphering off); present if set to "1" (ciphering on).
// note 2: If the "class of MS" or the "extended capabilities" element is not included and the SwMI needs either, it may accept the request and then send a D-LOCATION UPDATE COMMAND PDU.
#[derive(Debug)]
pub struct ULocationUpdateDemand {
    /// Type1, 3 bits, Location update type
    pub location_update_type: LocationUpdateType,
    /// Type1, 1 bits, Request to append LA
    pub request_to_append_la: bool,
    /// Type1, 1 bits, Cipher control
    pub cipher_control: bool,
    /// Conditional 10 bits, Ciphering parameters
    pub ciphering_parameters: Option<u64>,
    /// Type2, 24 bits, See note 2,
    pub class_of_ms: Option<u64>,
    /// Type2, 3 bits, Energy saving mode
    pub energy_saving_mode: Option<EnergySavingMode>,
    /// Type2, LA information
    pub la_information: Option<u64>,
    /// Type2, 24 bits, ISSI of the MS,
    pub ssi: Option<u64>,
    /// Type2, 24 bits, MNI of the MS,
    pub address_extension: Option<u64>,
    /// Type3, Group identity location demand
    pub group_identity_location_demand: Option<GroupIdentityLocationDemand>,
    /// Type3, 3 bits, Group report response
    pub group_report_response: Option<Type3FieldGeneric>,
    /// Type3, 3 bits, See ETSI EN 300 392-7 [8],
    pub authentication_uplink: Option<Type3FieldGeneric>,
    /// Type3, 3 bits, See note 2,
    pub extended_capabilities: Option<Type3FieldGeneric>,
    /// Type3, 3 bits, Proprietary
    pub proprietary: Option<Type3FieldGeneric>,
}

#[allow(unreachable_code)] // TODO FIXME review, finalize and remove this
impl ULocationUpdateDemand {
    /// Parse from BitBuffer
    pub fn from_bitbuf(buffer: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let pdu_type = buffer.read_field(4, "pdu_type")?;
        expect_pdu_type!(pdu_type, MmPduTypeUl::ULocationUpdateDemand)?;

        // Type1
        let val: u64 = buffer.read_field(3, "location_update_type")?;
        let result = LocationUpdateType::try_from(val);
        let location_update_type = match result {
            Ok(x) => x,
            Err(_) => {
                return Err(PduParseErr::InvalidValue {
                    field: "location_update_type",
                    value: val,
                });
            }
        };

        // Type1
        let request_to_append_la = buffer.read_field(1, "request_to_append_la")? != 0;
        // Type1
        let cipher_control = buffer.read_field(1, "cipher_control")? != 0;
        // Conditional
        let ciphering_parameters = if cipher_control {
            Some(buffer.read_field(10, "ciphering_parameters")?)
        } else {
            None
        };

        // obit designates presence of any further type2, type3 or type4 fields
        let mut obit = delimiters::read_obit(buffer)?;

        // Type2
        let class_of_ms = typed::parse_type2_generic(obit, buffer, 24, "class_of_ms")?;
        // Type2
        let val = typed::parse_type2_generic(obit, buffer, 3, "energy_saving_mode")?;
        let energy_saving_mode = match val {
            Some(v) => Some(EnergySavingMode::try_from(v).unwrap()), // Never fails
            None => None,
        };
        // Type2
        let la_information = typed::parse_type2_generic(obit, buffer, 15, "la_information")?;
        let la_information = match la_information {
            Some(v) => {
                // Most likely, this is 14-bits for the LA, then one zero-bit
                tracing::warn!("LA Information parsing not implemented/validated fully");
                Some(v / 2) // Remove trailing zero bit
            }
            None => None,
        };

        // Type2
        let ssi = typed::parse_type2_generic(obit, buffer, 24, "ssi")?;
        // Type2
        let address_extension = typed::parse_type2_generic(obit, buffer, 24, "address_extension")?;

        // Type3
        let group_identity_location_demand = typed::parse_type3_struct(
            obit,
            buffer,
            MmType34ElemIdUl::GroupIdentityLocationDemand,
            GroupIdentityLocationDemand::from_bitbuf,
        )?;

        // Type3
        let group_report_response = typed::parse_type3_generic(obit, buffer, MmType34ElemIdUl::GroupReportResponse)?;

        // Type3
        let authentication_uplink = typed::parse_type3_generic(obit, buffer, MmType34ElemIdUl::AuthenticationUplink)?;

        // Type3
        let extended_capabilities = typed::parse_type3_generic(obit, buffer, MmType34ElemIdUl::ExtendedCapabilities)?;

        // Type3
        let proprietary = typed::parse_type3_generic(obit, buffer, MmType34ElemIdUl::Proprietary)?;

        // Read trailing mbit (if not previously encountered)
        obit = if obit { buffer.read_field(1, "trailing_obit")? == 1 } else { obit };
        if obit {
            return Err(PduParseErr::InvalidTrailingMbitValue);
        }

        Ok(ULocationUpdateDemand {
            location_update_type,
            request_to_append_la,
            cipher_control,
            ciphering_parameters,
            class_of_ms,
            energy_saving_mode,
            la_information,
            ssi,
            address_extension,
            group_identity_location_demand,
            group_report_response,
            authentication_uplink,
            extended_capabilities,
            proprietary,
        })
    }

    /// Serialize this PDU into the given BitBuffer.
    pub fn to_bitbuf(&self, buffer: &mut BitBuffer) -> Result<(), PduParseErr> {
        // PDU Type
        buffer.write_bits(MmPduTypeUl::ULocationUpdateDemand.into_raw(), 4);
        // Type1
        buffer.write_bits(self.location_update_type as u64, 3);
        // Type1
        buffer.write_bits(self.request_to_append_la as u64, 1);
        // Type1
        buffer.write_bits(self.cipher_control as u64, 1);
        // Conditional
        if let Some(ref value) = self.ciphering_parameters {
            buffer.write_bits(*value, 10);
        }

        // Check if any optional field present and place o-bit
        let obit = self.class_of_ms.is_some()
            || self.energy_saving_mode.is_some()
            || self.la_information.is_some()
            || self.ssi.is_some()
            || self.address_extension.is_some()
            || self.group_identity_location_demand.is_some()
            || self.group_report_response.is_some()
            || self.authentication_uplink.is_some()
            || self.extended_capabilities.is_some()
            || self.proprietary.is_some();
        delimiters::write_obit(buffer, obit as u8);
        if !obit {
            return Ok(());
        }

        // Type2
        typed::write_type2_generic(obit, buffer, self.class_of_ms, 24);

        // Type2
        typed::write_type2_generic(obit, buffer, self.energy_saving_mode.map(|esm| esm.into()), 3);

        // Type2
        let la_and_zero_bit = if let Some(la) = self.la_information {
            tracing::warn!("LA Information serialization not implemented/validated fully");
            // Most likely, this is 14-bits for the LA, then one zero-bit
            Some(la << 1)
        } else {
            None
        };
        typed::write_type2_generic(obit, buffer, la_and_zero_bit, 15);

        // Type2
        typed::write_type2_generic(obit, buffer, self.ssi, 24);

        // Type2
        typed::write_type2_generic(obit, buffer, self.address_extension, 24);

        // Type3
        typed::write_type3_struct(
            obit,
            buffer,
            &self.group_identity_location_demand,
            MmType34ElemIdUl::GroupIdentityLocationDemand,
            GroupIdentityLocationDemand::to_bitbuf,
        )?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.group_report_response, MmType34ElemIdUl::GroupReportResponse)?;

        // Type3
        typed::write_type3_generic(obit, buffer, &self.authentication_uplink, MmType34ElemIdUl::AuthenticationUplink)?;

        // Type3

        typed::write_type3_generic(obit, buffer, &self.extended_capabilities, MmType34ElemIdUl::ExtendedCapabilities)?;

        // Type3

        typed::write_type3_generic(obit, buffer, &self.proprietary, MmType34ElemIdUl::Proprietary)?;

        // Write terminating m-bit
        delimiters::write_mbit(buffer, 0);
        Ok(())
    }
}

impl fmt::Display for ULocationUpdateDemand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ULocationUpdateDemand {{ location_update_type: {:?} request_to_append_la: {:?} cipher_control: {:?} ciphering_parameters: {:?} class_of_ms: {:?} energy_saving_mode: {:?} la_information: {:?} ssi: {:?} address_extension: {:?} group_identity_location_demand: {:?} group_report_response: {:?} authentication_uplink: {:?} extended_capabilities: {:?} proprietary: {:?} }}",
            self.location_update_type,
            self.request_to_append_la,
            self.cipher_control,
            self.ciphering_parameters,
            self.class_of_ms,
            self.energy_saving_mode,
            self.la_information,
            self.ssi,
            self.address_extension,
            self.group_identity_location_demand,
            self.group_report_response,
            self.authentication_uplink,
            self.extended_capabilities,
            self.proprietary,
        )
    }
}

#[cfg(test)]
mod tests {

    use tetra_core::debug;

    use super::*;

    #[test]
    fn test_u_location_update_demand_with_gild() {
        // Example of nested type3 struct that embeds another type4
        // Parsing group_identity_location_demand: 001000000110001001001010000001000000000^1001100000111000001110000000010010000000101000000000000000000000001101000
        // Parsing GroupIdentityLocationDemand:    00100000011000100100101000000100000000010011 00000111000 001^1 1000 00000100100 000001 01000000000000000000000001101000
        //         GroupIdentityLocationDemand:    00100000011000100100101000000100000000010011 00000111000 001 1 1000 00000100100 000001 010000000000000000000000011010^00
        // 00100000011000100100101000000100000000010011 00000111000 001 1 1000 00000100100 000001 010000000000000000000000011010 0^0
        //                                        | obit: fields follow
        //                                            |--| GroupIdentityLocationDemand ID
        //                                              |---------| GroupIdentityLocationDemand subelem len
        //                                                          || reserved, attach/detach bit
        //                                                            | obit: fields follow
        //                                                              ---------------------------------------------------------- GroupIdentityUplink
        //                                                              m -ID- -- len ---- - num- -data-                         m m(for upper struct)

        // Vec from moto upon registration

        debug::setup_logging_verbose();
        let test_vec = "0010000001100010010010100000010000000001001100000111000001110000000010010000000101000000000000000000000001101000";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = ULocationUpdateDemand::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);

        assert!(buf_in.get_len_remaining() == 0, "Buffer not fully consumed");
        let gild = pdu.group_identity_location_demand.unwrap();
        assert_eq!(gild.group_identity_attach_detach_mode, 0);
        let gild_giu = gild.group_identity_uplink.unwrap();
        assert_eq!(gild_giu.len(), 1);
        let giu0 = &gild_giu[0];
        assert_eq!(giu0.gssi, Some(26));
    }

    #[test]
    fn test_u_location_update_demand_with_gild_and_esm() {
        // Vec from moto upon registration
        // Contains optional energy_saving_mode and group_identity_location_demand

        debug::setup_logging_verbose();
        let test_vec =
            "0010000001100010010010100000010000010010001001100000111000001110000000010010000000101000000000000000000000001101000";
        let mut buf_in = BitBuffer::from_bitstr(test_vec);
        let pdu = ULocationUpdateDemand::from_bitbuf(&mut buf_in).expect("Failed parsing");

        tracing::info!("Parsed: {:?}", pdu);
        tracing::info!("Buf at end: {}", buf_in.dump_bin());

        let mut buf_out = BitBuffer::new_autoexpand(32);
        pdu.to_bitbuf(&mut buf_out).unwrap();
        tracing::info!("Serialized: {}", buf_out.dump_bin());
        assert_eq!(buf_out.to_bitstr(), test_vec);

        assert!(buf_in.get_len_remaining() == 0, "Buffer not fully consumed");
        let gild = pdu.group_identity_location_demand.unwrap();
        assert_eq!(gild.group_identity_attach_detach_mode, 0);
        let gild_giu = gild.group_identity_uplink.unwrap();
        assert_eq!(gild_giu.len(), 1);
        let giu0 = &gild_giu[0];
        assert_eq!(giu0.gssi, Some(26));
    }
}
