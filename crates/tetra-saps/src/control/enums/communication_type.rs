/// 14.8.17c Communication type
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommunicationType {
    /// Point-to-point
    P2p = 0,
    /// Point-to-multipoint
    P2Mp = 1,
    /// Point-to-multipoint Acknowledged
    P2MpAcked = 2,
    /// Broadcast
    Broadcast = 3,
}

impl std::convert::TryFrom<u64> for CommunicationType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(CommunicationType::P2p),
            1 => Ok(CommunicationType::P2Mp),
            2 => Ok(CommunicationType::P2MpAcked),
            3 => Ok(CommunicationType::Broadcast),
            _ => Err(()),
        }
    }
}

impl CommunicationType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CommunicationType::P2p => 0,
            CommunicationType::P2Mp => 1,
            CommunicationType::P2MpAcked => 2,
            CommunicationType::Broadcast => 3,
        }
    }
}

impl From<CommunicationType> for u64 {
    fn from(e: CommunicationType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CommunicationType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CommunicationType::P2p => write!(f, "P2p"),
            CommunicationType::P2Mp => write!(f, "P2mp"),
            CommunicationType::P2MpAcked => write!(f, "P2mpacked"),
            CommunicationType::Broadcast => write!(f, "Broadcast"),
        }
    }
}
