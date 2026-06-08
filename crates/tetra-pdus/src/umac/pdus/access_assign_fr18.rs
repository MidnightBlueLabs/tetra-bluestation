use crate::umac::pdus::access_assign::{AccessCode, BaseFrameLength};
use crate::umac::{enums::access_assign_ul_usage::AccessAssignUlUsage, pdus::access_assign::AccessField};
use core::fmt;
use std::panic;
use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

/// Clause 21.4.7.2 ACCESS-ASSIGN
#[derive(Debug)]
pub enum AccessAssignFr18 {
    // Header = 00
    // Uplink access rights - common only
    UplinkCommonOnly {
        access_field_1: AccessField,
        access_field_2: AccessField,
    },

    // Header = 01
    // Uplink access rights - common and assigned
    UplinkCommonAndAssigned {
        access_field_1: AccessField,
        access_field_2: AccessField,
    },

    // Header = 10
    // Uplink access rights - assigned only
    UplinkAssignedOnly {
        access_field_1: AccessField,
        access_field_2: AccessField,
    },

    // Header = 11
    // Uplink access rights - common and assigned, but with traffic usage marker (UMt) instead of AF1
    UplinkCommonAndAssignedTraffic {
        uplink_usage_marker: AccessAssignUlUsage,
        access_field: AccessField,
    },
}

impl Default for AccessAssignFr18 {
    fn default() -> Self {
        AccessAssignFr18::UplinkCommonOnly {
            access_field_1: AccessField {
                access_code: AccessCode::AccessCodeA,
                base_frame_len: BaseFrameLength::ReservedSubslot,
            },
            access_field_2: AccessField {
                access_code: AccessCode::AccessCodeA,
                base_frame_len: BaseFrameLength::ReservedSubslot,
            },
        }
    }
}

impl AccessAssignFr18 {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let header = buf.read_field(2, "_header")?;
        let field1 = buf.read_field(6, "field1")?;
        let field2 = buf.read_field(6, "field2")?;

        match header {
            0b00 => {
                // Uplink access rights - common only
                Ok(AccessAssignFr18::UplinkCommonOnly {
                    access_field_1: field1.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field_1",
                        value: field1,
                    })?,
                    access_field_2: field2.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field_2",
                        value: field2,
                    })?,
                })
            }

            0b01 => {
                // Uplink access rights - common and assigned
                Ok(AccessAssignFr18::UplinkCommonAndAssigned {
                    access_field_1: field1.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field_1",
                        value: field1,
                    })?,
                    access_field_2: field2.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field_2",
                        value: field2,
                    })?,
                })
            }

            0b10 => {
                // Uplink access rights - assigned only
                Ok(AccessAssignFr18::UplinkAssignedOnly {
                    access_field_1: field1.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field_1",
                        value: field1,
                    })?,
                    access_field_2: field2.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field_2",
                        value: field2,
                    })?,
                })
            }

            0b11 => {
                // Uplink access rights - common and assigned, but with traffic usage marker (UMt) instead of AF1
                let uplink_usage_marker = AccessAssignUlUsage::from_usage_marker(field1 as u8).ok_or(PduParseErr::InvalidValue {
                    field: "uplink_usage_marker",
                    value: field1,
                })?;
                // Table 21.82 mandates UMt for header=11. UMx and other non-traffic markers are forbidden.
                if !uplink_usage_marker.is_traffic() {
                    return Err(PduParseErr::InvalidValue {
                        field: "uplink_usage_marker",
                        value: field1,
                    });
                }
                Ok(AccessAssignFr18::UplinkCommonAndAssignedTraffic {
                    uplink_usage_marker,
                    access_field: field2.try_into().map_err(|_| PduParseErr::InvalidValue {
                        field: "access_field",
                        value: field2,
                    })?,
                })
            }

            _ => {
                panic!("Invalid header value for Frame 18 ACCESS-ASSIGN: {}", header);
            }
        }
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        match self {
            AccessAssignFr18::UplinkCommonOnly {
                access_field_1,
                access_field_2,
            } => {
                // Header = 00
                buf.write_bits(0b00, 2);

                // Access field 1
                buf.write_bits(access_field_1.into_raw(), 6);

                // Access field 2
                buf.write_bits(access_field_2.into_raw(), 6);
            }

            AccessAssignFr18::UplinkCommonAndAssigned {
                access_field_1,
                access_field_2,
            } => {
                // Header = 01
                buf.write_bits(0b01, 2);

                // Access field 1
                buf.write_bits(access_field_1.into_raw(), 6);

                // Access field 2
                buf.write_bits(access_field_2.into_raw(), 6);
            }

            AccessAssignFr18::UplinkAssignedOnly {
                access_field_1,
                access_field_2,
            } => {
                // Header = 10
                buf.write_bits(0b10, 2);

                // Access field 1
                buf.write_bits(access_field_1.into_raw(), 6);

                // Access field 2
                buf.write_bits(access_field_2.into_raw(), 6);
            }

            AccessAssignFr18::UplinkCommonAndAssignedTraffic {
                uplink_usage_marker,
                access_field,
            } => {
                // Header = 11
                buf.write_bits(0b11, 2);

                // Uplink usage marker
                buf.write_bits(uplink_usage_marker.to_usage_marker().unwrap() as u64, 6);

                // Access field (both subslots)
                buf.write_bits(access_field.into_raw(), 6);
            }
        }
    }
}

impl fmt::Display for AccessAssignFr18 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessAssignFr18::UplinkCommonOnly {
                access_field_1,
                access_field_2,
            } => {
                write!(
                    f,
                    "AccessAssignFr18 {{ UplinkCommonOnly: af1: {}, af2: {} }}",
                    access_field_1, access_field_2
                )
            }
            AccessAssignFr18::UplinkCommonAndAssigned {
                access_field_1,
                access_field_2,
            } => {
                write!(
                    f,
                    "AccessAssignFr18 {{ UplinkCommonAndAssigned: af1: {}, af2: {} }}",
                    access_field_1, access_field_2
                )
            }
            AccessAssignFr18::UplinkAssignedOnly {
                access_field_1,
                access_field_2,
            } => {
                write!(
                    f,
                    "AccessAssignFr18 {{ UplinkAssignedOnly: af1: {}, af2: {} }}",
                    access_field_1, access_field_2
                )
            }
            AccessAssignFr18::UplinkCommonAndAssignedTraffic {
                uplink_usage_marker,
                access_field,
            } => {
                write!(
                    f,
                    "AccessAssignFr18 {{ UplinkCommonAndAssignedTraffic: uum: {}, af: {} }}",
                    uplink_usage_marker, access_field
                )
            }
        }
    }
}
