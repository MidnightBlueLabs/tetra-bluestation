/// 14.8.17a Circuit mode type
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChanAllocType {
    Replace = 0,
    Additional = 1,
    QuitAndGo = 2,
    /// Replace current channel with specified channel plus carrier specific signalling channel in slot 1
    ReplaceWithCarrierSignalling = 3,
}

impl std::convert::TryFrom<u64> for ChanAllocType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(ChanAllocType::Replace),
            1 => Ok(ChanAllocType::Additional),
            2 => Ok(ChanAllocType::QuitAndGo),
            3 => Ok(ChanAllocType::ReplaceWithCarrierSignalling),
            _ => Err(()),
        }
    }
}

impl ChanAllocType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            ChanAllocType::Replace => 0,
            ChanAllocType::Additional => 1,
            ChanAllocType::QuitAndGo => 2,
            ChanAllocType::ReplaceWithCarrierSignalling => 3,
        }
    }
}

impl From<ChanAllocType> for u64 {
    fn from(e: ChanAllocType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for ChanAllocType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ChanAllocType::Replace => write!(f, "Replace"),
            ChanAllocType::Additional => write!(f, "Additional"),
            ChanAllocType::QuitAndGo => write!(f, "QuitAndGo"),
            ChanAllocType::ReplaceWithCarrierSignalling => write!(f, "ReplaceWithCarrierSignalling"),
        }
    }
}
