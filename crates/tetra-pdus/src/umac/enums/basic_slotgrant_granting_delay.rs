/// Clause 21.5.6 Basic slot granting, granting delay element
/// Bits: 4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BasicSlotgrantGrantingDelay {
    CapAllocAtNextOpportunity = 0,
    /// Delay N opportunities, where N is in the range 1..=13
    DelayNOpportunities(u8),
    AllocStartsAtOpportunityInFr18 = 14,
    WaitForAnotherSlotgrantMessage = 15,
}

impl std::convert::TryFrom<u64> for BasicSlotgrantGrantingDelay {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(BasicSlotgrantGrantingDelay::CapAllocAtNextOpportunity),
            1..=13 => Ok(BasicSlotgrantGrantingDelay::DelayNOpportunities(x as u8)),
            14 => Ok(BasicSlotgrantGrantingDelay::AllocStartsAtOpportunityInFr18),
            15 => Ok(BasicSlotgrantGrantingDelay::WaitForAnotherSlotgrantMessage),
            _ => Err(()),
        }
    }
}

impl BasicSlotgrantGrantingDelay {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            BasicSlotgrantGrantingDelay::CapAllocAtNextOpportunity => 0,
            BasicSlotgrantGrantingDelay::DelayNOpportunities(n) => n as u64,
            BasicSlotgrantGrantingDelay::AllocStartsAtOpportunityInFr18 => 14,
            BasicSlotgrantGrantingDelay::WaitForAnotherSlotgrantMessage => 15,
        }
    }
}

impl From<BasicSlotgrantGrantingDelay> for u64 {
    fn from(e: BasicSlotgrantGrantingDelay) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for BasicSlotgrantGrantingDelay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BasicSlotgrantGrantingDelay::CapAllocAtNextOpportunity => write!(f, "CapAllocAtNextOpportunity"),
            BasicSlotgrantGrantingDelay::DelayNOpportunities(n) => write!(f, "Delay{}Opportunities", n),
            BasicSlotgrantGrantingDelay::AllocStartsAtOpportunityInFr18 => write!(f, "AllocStartsAtOpportunityInFr18"),
            BasicSlotgrantGrantingDelay::WaitForAnotherSlotgrantMessage => write!(f, "WaitForAnotherSlotgrantMessage"),
        }
    }
}
