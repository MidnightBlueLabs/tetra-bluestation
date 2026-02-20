/// 21.5.2 Channel allocation
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UlDlAssignment {
    Augmented = 0,
    Dl = 1,
    Ul = 2,
    Both = 3,
}

impl std::convert::TryFrom<u64> for UlDlAssignment {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(UlDlAssignment::Augmented),
            1 => Ok(UlDlAssignment::Dl),
            2 => Ok(UlDlAssignment::Ul),
            3 => Ok(UlDlAssignment::Both),
            _ => Err(()),
        }
    }
}

impl UlDlAssignment {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            UlDlAssignment::Augmented => 0,
            UlDlAssignment::Dl => 1,
            UlDlAssignment::Ul => 2,
            UlDlAssignment::Both => 3,
        }
    }
}

impl From<UlDlAssignment> for u64 {
    fn from(e: UlDlAssignment) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for UlDlAssignment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UlDlAssignment::Augmented => write!(f, "Augmented"),
            UlDlAssignment::Dl => write!(f, "Dl"),
            UlDlAssignment::Ul => write!(f, "Ul"),
            UlDlAssignment::Both => write!(f, "Both"),
        }
    }
}
