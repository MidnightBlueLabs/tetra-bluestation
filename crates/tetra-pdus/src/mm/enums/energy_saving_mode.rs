/// Clause 16.10.9 Energy saving mode
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EnergySavingMode {
    StayAlive = 0,
    /// Economy Mode 1
    Eg1 = 1,
    /// Economy Mode 2
    Eg2 = 2,
    /// Economy Mode 3
    Eg3 = 3,
    /// Economy Mode 4
    Eg4 = 4,
    /// Economy Mode 5
    Eg5 = 5,
    /// Economy Mode 6
    Eg6 = 6,
    /// Economy Mode 7
    Eg7 = 7,
}

impl std::convert::TryFrom<u64> for EnergySavingMode {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(EnergySavingMode::StayAlive),
            1 => Ok(EnergySavingMode::Eg1),
            2 => Ok(EnergySavingMode::Eg2),
            3 => Ok(EnergySavingMode::Eg3),
            4 => Ok(EnergySavingMode::Eg4),
            5 => Ok(EnergySavingMode::Eg5),
            6 => Ok(EnergySavingMode::Eg6),
            7 => Ok(EnergySavingMode::Eg7),
            _ => Err(()),
        }
    }
}

impl EnergySavingMode {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            EnergySavingMode::StayAlive => 0,
            EnergySavingMode::Eg1 => 1,
            EnergySavingMode::Eg2 => 2,
            EnergySavingMode::Eg3 => 3,
            EnergySavingMode::Eg4 => 4,
            EnergySavingMode::Eg5 => 5,
            EnergySavingMode::Eg6 => 6,
            EnergySavingMode::Eg7 => 7,
        }
    }
}

impl From<EnergySavingMode> for u64 {
    fn from(e: EnergySavingMode) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for EnergySavingMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            EnergySavingMode::StayAlive => write!(f, "StayAlive"),
            EnergySavingMode::Eg1 => write!(f, "Eg1"),
            EnergySavingMode::Eg2 => write!(f, "Eg2"),
            EnergySavingMode::Eg3 => write!(f, "Eg3"),
            EnergySavingMode::Eg4 => write!(f, "Eg4"),
            EnergySavingMode::Eg5 => write!(f, "Eg5"),
            EnergySavingMode::Eg6 => write!(f, "Eg6"),
            EnergySavingMode::Eg7 => write!(f, "Eg7"),
        }
    }
}
