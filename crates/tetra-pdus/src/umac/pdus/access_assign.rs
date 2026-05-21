use core::fmt;
use std::panic;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};
use crate::umac::enums::{access_assign_dl_usage::AccessAssignDlUsage, access_assign_ul_usage::AccessAssignUlUsage};
pub(crate) use crate::umac::enums::access_code::AccessCode;
pub(crate) use crate::umac::structs::access_field::AccessField;
pub(crate) use crate::umac::structs::base_frame_length::BaseFrameLength;

/// Clause 21.4.7.2 ACCESS-ASSIGN
#[derive(Debug)]
pub enum AccessAssign {

    // Header = 00
    // Downlink usage - common control
    // Uplink access rights - common only
    DownlinkCommonControlUplinkCommonOnly {
        access_field_1: AccessField,
        access_field_2: AccessField
    },

    // Header = 01
    // Downlink usage - defined by field 1
    // Uplink access rights - common and assigned
    DownlinkDefinedUplinkCommonAndAssigned {
        downlink_usage_marker: AccessAssignDlUsage,
        access_field: AccessField
    },

    // Header = 10
    // Downlink usage - defined by field 1
    // Uplink access rights - assigned only
    DownlinkDefinedUplinkAssignedOnly {
        downlink_usage_marker: AccessAssignDlUsage,
        access_field: AccessField
    },

    // Header = 11
    // Downlink usage - defined by field 1
    // Uplink access rights - defined by field 2
    DownlinkDefinedUplinkDefined {
        downlink_usage_marker: AccessAssignDlUsage,
        uplink_usage_marker: AccessAssignUlUsage
    }

}

impl Default for AccessAssign {
    fn default() -> Self {
        AccessAssign::DownlinkCommonControlUplinkCommonOnly {
            access_field_1: AccessField {
                access_code: AccessCode::AccessCodeA,
                base_frame_len: BaseFrameLength::Subslots4
            },
            access_field_2: AccessField {
                access_code: AccessCode::AccessCodeA,
                base_frame_len: BaseFrameLength::Subslots4
             }
        }
    }
}

impl AccessAssign {

    pub fn dl_is_traffic(&self) -> bool {
        match self {
            AccessAssign::DownlinkDefinedUplinkCommonAndAssigned { downlink_usage_marker, .. } |
            AccessAssign::DownlinkDefinedUplinkAssignedOnly { downlink_usage_marker, .. } |
            AccessAssign::DownlinkDefinedUplinkDefined { downlink_usage_marker, .. } => {
                downlink_usage_marker.is_traffic()
            },
            _ => false
        }
    }

    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {

        let header = buf.read_field(2, "_header")?;
        let field1 = buf.read_field(6, "field1")?;
        let field2 = buf.read_field(6, "field2")?;

        match header {

            0b00 => {
                // Downlink usage - common control
                // Uplink access rights - common only
                Ok(AccessAssign::DownlinkCommonControlUplinkCommonOnly {
                    access_field_1: field1.try_into()
                        .map_err(|_| PduParseErr::InvalidValue { field: "access_field_1", value: field1 })?,
                    access_field_2: field2.try_into()
                        .map_err(|_| PduParseErr::InvalidValue { field: "access_field_2", value: field2 })?,
                })
            }

            0b01 => {
                // Downlink usage - defined by field 1
                // Uplink access rights - common and assigned
                Ok(AccessAssign::DownlinkDefinedUplinkCommonAndAssigned {
                    downlink_usage_marker: AccessAssignDlUsage::from_usage_marker(field1 as u8),
                    access_field: field2.try_into()
                        .map_err(|_| PduParseErr::InvalidValue { field: "access_field", value: field2 })?,
                })
            }

            0b10 => {
                // Downlink usage - defined by field 1
                // Uplink access rights - assigned only
                Ok(AccessAssign::DownlinkDefinedUplinkAssignedOnly {
                    downlink_usage_marker: AccessAssignDlUsage::from_usage_marker(field1 as u8),
                    access_field: field2.try_into()
                        .map_err(|_| PduParseErr::InvalidValue { field: "access_field", value: field2 })?,
                })
            }

            0b11 => {
                // Downlink usage - defined by field 1
                // Uplink access rights - defined by field 2
                Ok(AccessAssign::DownlinkDefinedUplinkDefined {
                    downlink_usage_marker: AccessAssignDlUsage::from_usage_marker(field1 as u8),
                    uplink_usage_marker: AccessAssignUlUsage::from_usage_marker(field2 as u8).unwrap(),
                })
            }

            _ => {
                panic!("Invalid header value for ACCESS-ASSIGN: {}", header);
            }
        }
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {

        match self {

            AccessAssign::DownlinkCommonControlUplinkCommonOnly {
                access_field_1,
                access_field_2
            } => {

                // Header = 00
                buf.write_bits(0b00, 2);

                // Access field 1
                buf.write_bits(access_field_1.into_raw(), 6);

                // Access field 2
                buf.write_bits(access_field_2.into_raw(), 6);

            },

            AccessAssign::DownlinkDefinedUplinkCommonAndAssigned {
                downlink_usage_marker,
                access_field
            } => {

                // Header = 01
                buf.write_bits(0b01, 2);

                // Downlink usage marker
                buf.write_bits(downlink_usage_marker.to_usage_marker() as u64, 6);

                // Access field (both subslots)
                buf.write_bits(access_field.into_raw(), 6);

            },

            AccessAssign::DownlinkDefinedUplinkAssignedOnly {
                downlink_usage_marker,
                access_field
            } => {

                // Header = 10
                buf.write_bits(0b10, 2);

                // Downlink usage marker
                buf.write_bits(downlink_usage_marker.to_usage_marker() as u64, 6);

                // Access field (both subslots)
                buf.write_bits(access_field.into_raw(), 6);

            },

            AccessAssign::DownlinkDefinedUplinkDefined {
                downlink_usage_marker,
                uplink_usage_marker
            } => {

                // Header = 11
                buf.write_bits(0b11, 2);

                // Downlink usage marker
                buf.write_bits(downlink_usage_marker.to_usage_marker() as u64, 6);

                // Uplink usage marker
                buf.write_bits(uplink_usage_marker.to_usage_marker().unwrap() as u64, 6);

            }
        }
    }
}

impl fmt::Display for AccessAssign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessAssign::DownlinkCommonControlUplinkCommonOnly { access_field_1, access_field_2 } => {
                write!(f, "AccessAssign {{ DL: Common Ctrl, UL: Common Only, af1: {}, af2: {} }}", access_field_1, access_field_2)
            },
            AccessAssign::DownlinkDefinedUplinkCommonAndAssigned { downlink_usage_marker, access_field } => {
                write!(f, "AccessAssign {{ DL: {}, UL: Common & Assigned, af: {} }}", downlink_usage_marker, access_field)
            },
            AccessAssign::DownlinkDefinedUplinkAssignedOnly { downlink_usage_marker, access_field } => {
                write!(f, "AccessAssign {{ DL: {}, UL: Assigned Only, af: {} }}", downlink_usage_marker, access_field)
            },
            AccessAssign::DownlinkDefinedUplinkDefined { downlink_usage_marker, uplink_usage_marker } => {
                write!(f, "AccessAssign {{ DL: {}, UL: {} }}", downlink_usage_marker, uplink_usage_marker)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unallocated() {
        let bitstr = "11000000000000";
        let mut buf = BitBuffer::from_bitstr(bitstr);
        let mut bitarr = [0_u8; 14];
        let mut new_bitarr = [0_u8; 14];
        buf.to_bitarr(&mut bitarr);
        buf.seek(0);
        println!("buf: {}", buf.dump_bin());
        let pdu = AccessAssign::from_bitbuf(&mut buf).unwrap();
        println!("pdu: {:?}", pdu);
        let mut new_buf = BitBuffer::new(14);
        pdu.to_bitbuf(&mut new_buf);
        new_buf.seek(0);
        new_buf.to_bitarr(&mut new_bitarr);
        println!("new: {:?}", new_buf.dump_bin());
        assert_eq!(bitarr, new_bitarr);
    }

    #[test]
    fn test_commoncontrol() {
        let bitstr = "00001010001010";
        let mut buf = BitBuffer::from_bitstr(bitstr);
        let mut bitarr = [0_u8; 14];
        let mut new_bitarr = [0_u8; 14];
        buf.to_bitarr(&mut bitarr);
        buf.seek(0);
        println!("buf: {}", buf.dump_bin());
        let pdu = AccessAssign::from_bitbuf(&mut buf).unwrap();
        println!("pdu: {:?}", pdu);
        let mut new_buf = BitBuffer::new(14);
        pdu.to_bitbuf(&mut new_buf);
        new_buf.seek(0);
        new_buf.to_bitarr(&mut new_bitarr);
        println!("new: {:?}", new_buf.dump_bin());
        assert_eq!(bitarr, new_bitarr);
    }
}
