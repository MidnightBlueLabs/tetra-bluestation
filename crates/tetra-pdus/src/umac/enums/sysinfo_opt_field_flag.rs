/// Clause 21.4.4.1 Table 21.65
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SysinfoOptFieldFlag {
    /// Even multiframe definition for TS mode
    EvenMfDefForTsMode = 0,
    /// Odd multiframe definition for TS mode
    OddMfDefForTsMode = 1,
    /// Default definition for access code A
    DefaultDefForAccCodeA = 2,
    /// Extended services broadcast
    ExtServicesBroadcast = 3,
}

impl std::convert::TryFrom<u64> for SysinfoOptFieldFlag {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(SysinfoOptFieldFlag::EvenMfDefForTsMode),
            1 => Ok(SysinfoOptFieldFlag::OddMfDefForTsMode),
            2 => Ok(SysinfoOptFieldFlag::DefaultDefForAccCodeA),
            3 => Ok(SysinfoOptFieldFlag::ExtServicesBroadcast),
            _ => Err(()),
        }
    }
}

impl SysinfoOptFieldFlag {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            SysinfoOptFieldFlag::EvenMfDefForTsMode => 0,
            SysinfoOptFieldFlag::OddMfDefForTsMode => 1,
            SysinfoOptFieldFlag::DefaultDefForAccCodeA => 2,
            SysinfoOptFieldFlag::ExtServicesBroadcast => 3,
        }
    }
}

impl From<SysinfoOptFieldFlag> for u64 {
    fn from(e: SysinfoOptFieldFlag) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for SysinfoOptFieldFlag {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SysinfoOptFieldFlag::EvenMfDefForTsMode => write!(f, "EvenMfDefForTsMode"),
            SysinfoOptFieldFlag::OddMfDefForTsMode => write!(f, "OddMfDefForTsMode"),
            SysinfoOptFieldFlag::DefaultDefForAccCodeA => write!(f, "DefaultDefForAccCodeA"),
            SysinfoOptFieldFlag::ExtServicesBroadcast => write!(f, "ExtServicesBroadcast"),
        }
    }
}
