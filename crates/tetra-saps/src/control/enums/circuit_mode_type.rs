/// 14.8.17a Circuit mode type
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitModeType {
    /// Tch/S
    TchS = 0,
    /// Tch/7.2
    Tch72 = 1,
    /// Tch/4.8 N=1
    Tch48n1 = 2,
    /// Tch/4.8 N=4
    Tch48n4 = 3,
    /// Tch/4.8 N=8
    Tch48n8 = 4,
    /// Tch/2.4 N=1
    Tch24n1 = 5,
    /// Tch/2.4 N=4
    Tch24n4 = 6,
    /// Tch/2.4 N=8
    Tch24n8 = 7,
}

impl std::convert::TryFrom<u64> for CircuitModeType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(CircuitModeType::TchS),
            1 => Ok(CircuitModeType::Tch72),
            2 => Ok(CircuitModeType::Tch48n1),
            3 => Ok(CircuitModeType::Tch48n4),
            4 => Ok(CircuitModeType::Tch48n8),
            5 => Ok(CircuitModeType::Tch24n1),
            6 => Ok(CircuitModeType::Tch24n4),
            7 => Ok(CircuitModeType::Tch24n8),
            _ => Err(()),
        }
    }
}

impl CircuitModeType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CircuitModeType::TchS => 0,
            CircuitModeType::Tch72 => 1,
            CircuitModeType::Tch48n1 => 2,
            CircuitModeType::Tch48n4 => 3,
            CircuitModeType::Tch48n8 => 4,
            CircuitModeType::Tch24n1 => 5,
            CircuitModeType::Tch24n4 => 6,
            CircuitModeType::Tch24n8 => 7,
        }
    }
}

impl From<CircuitModeType> for u64 {
    fn from(e: CircuitModeType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CircuitModeType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CircuitModeType::TchS => write!(f, "Tch/S"),
            CircuitModeType::Tch72 => write!(f, "Tch/7.2"),
            CircuitModeType::Tch48n1 => write!(f, "Tch/4.8 N=1"),
            CircuitModeType::Tch48n4 => write!(f, "Tch/4.8 N=4"),
            CircuitModeType::Tch48n8 => write!(f, "Tch/4.8 N=8"),
            CircuitModeType::Tch24n1 => write!(f, "Tch/2.4 N=1"),
            CircuitModeType::Tch24n4 => write!(f, "Tch/2.4 N=4"),
            CircuitModeType::Tch24n8 => write!(f, "Tch/2.4 N=8"),
        }
    }
}
