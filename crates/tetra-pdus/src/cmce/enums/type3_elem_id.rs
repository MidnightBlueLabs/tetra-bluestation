/// Clause 14.8.48 Type 3 element identifier
///
/// Bits: 4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CmceType3ElemId {
    Dtmf = 1,
    ExtSubscriberNum = 2,
    Facility = 3,
    PollResponseAddr = 4,
    TempAddr = 5,
    DmMsAddr = 6,
    Proprietary = 15,
}

impl std::convert::TryFrom<u64> for CmceType3ElemId {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            1 => Ok(CmceType3ElemId::Dtmf),
            2 => Ok(CmceType3ElemId::ExtSubscriberNum),
            3 => Ok(CmceType3ElemId::Facility),
            4 => Ok(CmceType3ElemId::PollResponseAddr),
            5 => Ok(CmceType3ElemId::TempAddr),
            6 => Ok(CmceType3ElemId::DmMsAddr),
            15 => Ok(CmceType3ElemId::Proprietary),
            _ => Err(()),
        }
    }
}

impl CmceType3ElemId {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CmceType3ElemId::Dtmf => 1,
            CmceType3ElemId::ExtSubscriberNum => 2,
            CmceType3ElemId::Facility => 3,
            CmceType3ElemId::PollResponseAddr => 4,
            CmceType3ElemId::TempAddr => 5,
            CmceType3ElemId::DmMsAddr => 6,
            CmceType3ElemId::Proprietary => 15,
        }
    }
}

impl From<CmceType3ElemId> for u64 {
    fn from(e: CmceType3ElemId) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CmceType3ElemId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CmceType3ElemId::Dtmf => write!(f, "Dtmf"),
            CmceType3ElemId::ExtSubscriberNum => write!(f, "ExtSubscriberNum"),
            CmceType3ElemId::Facility => write!(f, "Facility"),
            CmceType3ElemId::PollResponseAddr => write!(f, "PollResponseAddr"),
            CmceType3ElemId::TempAddr => write!(f, "TempAddr"),
            CmceType3ElemId::DmMsAddr => write!(f, "DmMsAddr"),
            CmceType3ElemId::Proprietary => write!(f, "Proprietary"),
        }
    }
}
