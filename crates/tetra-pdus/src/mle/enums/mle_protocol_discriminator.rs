/// Clause 18.5.21 Protocol discriminator
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MleProtocolDiscriminator {
    // RESERVED = 0,
    Mm = 1,
    Cmce = 2,
    // RESERVED = 3,
    Sndcp = 4,
    Mle = 5,
    TetraManagementEntity = 6,
    // ReservedForTesting = 7,
}

impl std::convert::TryFrom<u64> for MleProtocolDiscriminator {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            // 0 => Ok(MleProtocolDiscriminator::RESERVED),
            1 => Ok(MleProtocolDiscriminator::Mm),
            2 => Ok(MleProtocolDiscriminator::Cmce),
            // 3 => Ok(MleProtocolDiscriminator::RESERVED),
            4 => Ok(MleProtocolDiscriminator::Sndcp),
            5 => Ok(MleProtocolDiscriminator::Mle),
            6 => Ok(MleProtocolDiscriminator::TetraManagementEntity),
            // 7 => Ok(MleProtocolDiscriminator::ReservedForTesting),
            _ => Err(()),
        }
    }
}

impl MleProtocolDiscriminator {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            // MleProtocolDiscriminator::RESERVED => 0,
            MleProtocolDiscriminator::Mm => 1,
            MleProtocolDiscriminator::Cmce => 2,
            // MleProtocolDiscriminator::RESERVED => 3,
            MleProtocolDiscriminator::Sndcp => 4,
            MleProtocolDiscriminator::Mle => 5,
            MleProtocolDiscriminator::TetraManagementEntity => 6,
            // MleProtocolDiscriminator::ReservedForTesting => 7,
        }
    }
}

impl From<MleProtocolDiscriminator> for u64 {
    fn from(e: MleProtocolDiscriminator) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MleProtocolDiscriminator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            // MleProtocolDiscriminator::RESERVED => write!(f, "RESERVED"),
            MleProtocolDiscriminator::Mm => write!(f, "Mm"),
            MleProtocolDiscriminator::Cmce => write!(f, "Cmce"),
            // MleProtocolDiscriminator::RESERVED => write!(f, "RESERVED"),
            MleProtocolDiscriminator::Sndcp => write!(f, "Sndcp"),
            MleProtocolDiscriminator::Mle => write!(f, "Mle"),
            MleProtocolDiscriminator::TetraManagementEntity => write!(f, "TetraManagementEntity"),
            // MleProtocolDiscriminator::ReservedForTesting => write!(f, "ReservedForTesting"),
        }
    }
}
