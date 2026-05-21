use core::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseFrameLength {

    /// Essentially "already assigned" - Access Code is meaningless in this Access Field
    ReservedSubslot,

    /// CLCH opportunity - Access Code is meaningless in this Access Field (common access for linearisation only)
    CLCHSubslot,

    /// Ongoing Frame, a continuation of the ongoing access frame
    OngoingFrame,

    // The following options indicate the start of a new access frame
    Subslots1,
    Subslots2,
    Subslots3,
    Subslots4,
    Subslots5,
    Subslots6,
    Subslots8,
    Subslots10,
    Subslots12,
    Subslots16,
    Subslots20,
    Subslots24,
    Subslots32
}

impl TryFrom<u64> for BaseFrameLength {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0b0000 => Ok(BaseFrameLength::ReservedSubslot),
            0b0001 => Ok(BaseFrameLength::CLCHSubslot),
            0b0010 => Ok(BaseFrameLength::OngoingFrame),
            0b0011 => Ok(BaseFrameLength::Subslots1),
            0b0100 => Ok(BaseFrameLength::Subslots2),
            0b0101 => Ok(BaseFrameLength::Subslots3),
            0b0110 => Ok(BaseFrameLength::Subslots4),
            0b0111 => Ok(BaseFrameLength::Subslots5),
            0b1000 => Ok(BaseFrameLength::Subslots6),
            0b1001 => Ok(BaseFrameLength::Subslots8),
            0b1010 => Ok(BaseFrameLength::Subslots10),
            0b1011 => Ok(BaseFrameLength::Subslots12),
            0b1100 => Ok(BaseFrameLength::Subslots16),
            0b1101 => Ok(BaseFrameLength::Subslots20),
            0b1110 => Ok(BaseFrameLength::Subslots24),
            0b1111 => Ok(BaseFrameLength::Subslots32),
            _ => Err(()),
        }
    }
}

impl BaseFrameLength {
    pub fn into_raw(self) -> u64 {
        match self {
            BaseFrameLength::ReservedSubslot => 0b0000,
            BaseFrameLength::CLCHSubslot => 0b0001,
            BaseFrameLength::OngoingFrame => 0b0010,
            BaseFrameLength::Subslots1 => 0b0011,
            BaseFrameLength::Subslots2 => 0b0100,
            BaseFrameLength::Subslots3 => 0b0101,
            BaseFrameLength::Subslots4 => 0b0110,
            BaseFrameLength::Subslots5 => 0b0111,
            BaseFrameLength::Subslots6 => 0b1000,
            BaseFrameLength::Subslots8 => 0b1001,
            BaseFrameLength::Subslots10 => 0b1010,
            BaseFrameLength::Subslots12 => 0b1011,
            BaseFrameLength::Subslots16 => 0b1100,
            BaseFrameLength::Subslots20 => 0b1101,
            BaseFrameLength::Subslots24 => 0b1110,
            BaseFrameLength::Subslots32 => 0b1111
        }
    }
}

impl fmt::Display for BaseFrameLength {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BaseFrameLength::ReservedSubslot => write!(f, "Reserved Subslot"),
            BaseFrameLength::CLCHSubslot => write!(f, "CLCH Opportunity"),
            BaseFrameLength::OngoingFrame => write!(f, "Ongoing Frame"),
            BaseFrameLength::Subslots1 => write!(f, "1 Subslot"),
            BaseFrameLength::Subslots2 => write!(f, "2 Subslots"),
            BaseFrameLength::Subslots3 => write!(f, "3 Subslots"),
            BaseFrameLength::Subslots4 => write!(f, "4 Subslots"),
            BaseFrameLength::Subslots5 => write!(f, "5 Subslots"),
            BaseFrameLength::Subslots6 => write!(f, "6 Subslots"),
            BaseFrameLength::Subslots8 => write!(f, "8 Subslots"),
            BaseFrameLength::Subslots10 => write!(f, "10 Subslots"),
            BaseFrameLength::Subslots12 => write!(f, "12 Subslots"),
            BaseFrameLength::Subslots16 => write!(f, "16 Subslots"),
            BaseFrameLength::Subslots20 => write!(f, "20 Subslots"),
            BaseFrameLength::Subslots24 => write!(f, "24 Subslots"),
            BaseFrameLength::Subslots32 => write!(f, "32 Subslots")
        }
    }
}