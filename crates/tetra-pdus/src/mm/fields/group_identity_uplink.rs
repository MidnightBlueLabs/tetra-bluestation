use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

/// 16.10.27 Group identity uplink
#[derive(Debug, Clone)]
pub struct GroupIdentityUplink {
    // 1
    // pub attach_detach_type_identifier: bool,
    // 3 opt
    pub class_of_usage: Option<u8>,
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

impl GroupIdentityUplink {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = GroupIdentityUplink {
            // attach_detach_type_identifier: false,
            class_of_usage: None,
            group_identity_detachment_uplink: None,
            // group_identity_address_type: 0,
            gssi: None,
            address_extension: None,
            vgssi: None,
        };

        let attach_detach_type_identifier = buf.read_field(1, "attach_detach_type_identifier")?;
        if attach_detach_type_identifier == 0 {
            s.class_of_usage = Some(buf.read_field(3, "class_of_usage")? as u8);
        }
        if attach_detach_type_identifier == 1 {
            s.group_identity_detachment_uplink = Some(buf.read_field(2, "group_identity_detachment_uplink")? as u8);
        }

        let address_type = buf.read_field(2, "address_type")? as u8;
        if address_type == 0 || address_type == 1 {
            s.gssi = Some(buf.read_field(24, "gssi")? as u32);
        }
        if address_type == 1 {
            s.address_extension = Some(buf.read_field(24, "address_extension")? as u32);
        }
        if address_type == 2 {
            s.vgssi = Some(buf.read_field(24, "vgssi")? as u32);
        }

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) -> Result<(), PduParseErr> {
        assert!(
            self.class_of_usage.is_some() ^ self.group_identity_detachment_uplink.is_some(),
            "need one of class_of_usage or group_identity_detachment_uplink"
        );
        buf.write_bits(if self.class_of_usage.is_some() { 0 } else { 1 }, 1);
        if let Some(v) = self.class_of_usage {
            buf.write_bits(v as u64, 3);
        }
        if let Some(v) = self.group_identity_detachment_uplink {
            buf.write_bits(v as u64, 2);
        }

        let address_type = if self.gssi.is_some() {
            assert!(self.vgssi.is_none(), "vgssi should be None if gssi is Some");
            if self.address_extension.is_some() { 1 } else { 0 }
        } else {
            assert!(self.vgssi.is_some(), "vgssi should be Some if gssi is None");
            2
        };

        buf.write_bits(address_type as u64, 2);
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

impl fmt::Display for GroupIdentityUplink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "group_identity_uplink {{\n  class_of_usage: {:?}\n  group_identity_detachment_uplink: {:?}\n  gssi: {:?}\n  address_extension: {:?}\n  vgssi: {:?}\n}}\n",
            self.class_of_usage, self.group_identity_detachment_uplink, self.gssi, self.address_extension, self.vgssi,
        )
    }
}
