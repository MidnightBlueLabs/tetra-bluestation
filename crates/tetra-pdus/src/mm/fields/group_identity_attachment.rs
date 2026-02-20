use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

/// 16.10.19 Group Identity Attachment
#[derive(Debug, Clone)]
pub struct GroupIdentityAttachment {
    /// 2 bits.
    /// 0: Attachment not needed
    /// 1: Attachment for next ITSI attach required
    /// 2: Attachment not allowed for next ITSI attach
    /// 3: Attachment for next location update required (good default)
    pub group_identity_attachment_lifetime: u8,
    /// 3 bits
    pub class_of_usage: u8,
}

impl GroupIdentityAttachment {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let mut s = GroupIdentityAttachment {
            group_identity_attachment_lifetime: 0,
            class_of_usage: 0,
        };

        s.group_identity_attachment_lifetime = buf.read_field(2, "group_identity_attachment_lifetime")? as u8;
        s.class_of_usage = buf.read_field(3, "class_of_usage")? as u8;

        Ok(s)
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.group_identity_attachment_lifetime as u64, 2);
        buf.write_bits(self.class_of_usage as u64, 3);
    }
}

impl fmt::Display for GroupIdentityAttachment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "group_identity_attachment {{ group_identity_attachment_lifetime: {} class_of_usage: {} }}",
            self.group_identity_attachment_lifetime, self.class_of_usage
        )
    }
}
