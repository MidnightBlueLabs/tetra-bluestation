use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::mm::fields::group_identity_attachment::GroupIdentityAttachment;

/// 16.10.22 Group identity downlink
#[derive(Debug, Clone)]
pub struct GroupIdentityDownlink {
    // 1
    // pub attach_detach_type_identifier: u8,
    // 5 opt
    pub group_identity_attachment: Option<GroupIdentityAttachment>,
    // 2 opt
    pub group_identity_detachment_uplink: Option<u8>,
    // 2
    // pub group_identity_address_type: u8,
    // 24 opt
    pub gssi: Option<u32>,
    // 24 opt
    pub address_extension: Option<u32>,
    // 24 opt
    pub vgssi: Option<u32>,
}

impl GroupIdentityDownlink {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = GroupIdentityDownlink {
            // attach_detach_type_identifier: 0,
            group_identity_attachment: None,
            group_identity_detachment_uplink: None,
            // group_identity_address_type: 0,
            gssi: None,
            address_extension: None,
            vgssi: None,
        };

        let attach_detach_type_identifier = buf.read_field(1, "attach_detach_type_identifier")? as u8;
        if attach_detach_type_identifier == 0 {
            s.group_identity_attachment = Some(GroupIdentityAttachment::from_bitbuf(buf)?);
        }
        if attach_detach_type_identifier == 1 {
            s.group_identity_detachment_uplink = Some(buf.read_field(2, "attach_detach_type_identifier")? as u8);
        }

        let address_type = buf.read_field(2, "address_type")? as u8;
        if address_type == 0 || address_type == 1 || address_type == 3 {
            s.gssi = Some(buf.read_field(24, "gssi")? as u32);
        }
        if address_type == 1 || address_type == 3 {
            s.address_extension = Some(buf.read_field(24, "address_extension")? as u32);
        }
        if address_type == 2 || address_type == 3 {
            s.vgssi = Some(buf.read_field(24, "vgssi")? as u32);
        }

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) -> Result<(), PduParseErr> {
        assert!(
            self.group_identity_attachment.is_some() ^ self.group_identity_detachment_uplink.is_some(),
            "need one of group_identity_attachment or group_identity_detachment_uplink"
        );

        buf.write_bits(if self.group_identity_attachment.is_some() { 0 } else { 1 }, 1);
        if let Some(v) = &self.group_identity_attachment {
            v.to_bitbuf(buf);
        }
        if let Some(v) = self.group_identity_detachment_uplink {
            buf.write_bits(v as u64, 2);
        }

        let address_type = if self.gssi.is_some() {
            if self.address_extension.is_some() {
                if self.vgssi.is_some() { 3 } else { 1 }
            } else {
                if self.vgssi.is_some() {
                    Err(PduParseErr::Inconsistency {
                        field: "vgssi",
                        reason: "vgssi must be None if gssi is Some and address_extension is None",
                    })?;
                }
                0
            }
        } else {
            if self.address_extension.is_some() {
                Err(PduParseErr::Inconsistency {
                    field: "address_extension",
                    reason: "address_extension must be None if gssi is None",
                })?;
            }
            if self.vgssi.is_none() {
                return Err(PduParseErr::Inconsistency {
                    field: "vgssi",
                    reason: "vgssi must be Some if gssi is None",
                });
            }
            2
        };

        buf.write_bits(address_type, 2);
        if let Some(v) = self.gssi {
            buf.write_bits(v as u64, 24);
        }
        if let Some(v) = self.address_extension {
            buf.write_bits(v as u64, 24);
        }
        if let Some(v) = self.vgssi {
            buf.write_bits(v as u64, 24);
        }

        Ok(())
    }
}

impl fmt::Display for GroupIdentityDownlink {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "group_identity_downlink {{ group_identity_attachment: {:?} group_identity_detachment_uplink: {:?} gssi: {:?} address_extension: {:?} vgssi: {:?} }}",
            self.group_identity_attachment, self.group_identity_detachment_uplink, self.gssi, self.address_extension, self.vgssi
        )
    }
}
