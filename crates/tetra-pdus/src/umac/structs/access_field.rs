use core::fmt;
use crate::umac::enums::access_code::AccessCode;
use crate::umac::structs::base_frame_length::BaseFrameLength;

#[derive(Debug, Clone, Copy)]
pub struct AccessField {
    pub access_code: AccessCode,
    pub base_frame_len: BaseFrameLength,
}

impl TryFrom<u64> for AccessField {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(AccessField {
            access_code: ((value >> 4) & 0b11).try_into()?,
            base_frame_len: (value & 0b1111).try_into()?,
        })
    }
}

impl AccessField {
    pub fn into_raw(self) -> u64 {
        (self.access_code.into_raw() << 4) | self.base_frame_len.into_raw()
    }
}

impl fmt::Display for AccessField {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Access Code: {}, Base Frame Length: {}", self.access_code, self.base_frame_len)
    }
}

