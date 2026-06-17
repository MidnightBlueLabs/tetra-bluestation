use core::fmt;
use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum AccessCode {
    AccessCodeA,
    AccessCodeB,
    AccessCodeC,
    AccessCodeD,
}

impl TryFrom<u64> for AccessCode {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0b00 => Ok(AccessCode::AccessCodeA),
            0b01 => Ok(AccessCode::AccessCodeB),
            0b10 => Ok(AccessCode::AccessCodeC),
            0b11 => Ok(AccessCode::AccessCodeD),
            _ => Err(()),
        }
    }
}

impl AccessCode {
    pub fn into_raw(self) -> u64 {
        match self {
            AccessCode::AccessCodeA => 0b00,
            AccessCode::AccessCodeB => 0b01,
            AccessCode::AccessCodeC => 0b10,
            AccessCode::AccessCodeD => 0b11,
        }
    }
}

impl Display for AccessCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AccessCode::AccessCodeA => write!(f, "Access Code A"),
            AccessCode::AccessCodeB => write!(f, "Access Code B"),
            AccessCode::AccessCodeC => write!(f, "Access Code C"),
            AccessCode::AccessCodeD => write!(f, "Access Code D"),
        }
    }
}
