/// Clause 21.4.4.0 Table 21.64
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BroadcastType {
    /// SYSINFO PDU if sent using π/4-DQPSK modulation or π/8-D8PSK modulation; or SYSINFO-Q PDU if sent using QAM modulation
    Sysinfo = 0,
    /// ACCESS-DEFINE PDU
    AccessDefine = 1,
    /// SYSINFO-DA
    SysinfoDa = 2,
}

impl std::convert::TryFrom<u64> for BroadcastType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(BroadcastType::Sysinfo),
            1 => Ok(BroadcastType::AccessDefine),
            2 => Ok(BroadcastType::SysinfoDa),
            _ => Err(()),
        }
    }
}

impl BroadcastType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            BroadcastType::Sysinfo => 0,
            BroadcastType::AccessDefine => 1,
            BroadcastType::SysinfoDa => 2,
        }
    }
}

impl From<BroadcastType> for u64 {
    fn from(e: BroadcastType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for BroadcastType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BroadcastType::Sysinfo => write!(f, "Sysinfo"),
            BroadcastType::AccessDefine => write!(f, "AccessDefine"),
            BroadcastType::SysinfoDa => write!(f, "SysinfoDa"),
        }
    }
}
