/// Clause 21.4.3.1 Table 21.55 MAC-RESOURCE address types
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MacResourceAddrType {
    NullPdu = 0,
    Ssi = 1,
    EventLabel = 2,
    Ussi = 3,
    Smi = 4,
    SsiAndEventLabel = 5,
    SsiAndUsageMarker = 6,
    SmiAndEventLabel = 7,
}

impl std::convert::TryFrom<u64> for MacResourceAddrType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(MacResourceAddrType::NullPdu),
            1 => Ok(MacResourceAddrType::Ssi),
            2 => Ok(MacResourceAddrType::EventLabel),
            3 => Ok(MacResourceAddrType::Ussi),
            4 => Ok(MacResourceAddrType::Smi),
            5 => Ok(MacResourceAddrType::SsiAndEventLabel),
            6 => Ok(MacResourceAddrType::SsiAndUsageMarker),
            7 => Ok(MacResourceAddrType::SmiAndEventLabel),
            _ => Err(()),
        }
    }
}

impl MacResourceAddrType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MacResourceAddrType::NullPdu => 0,
            MacResourceAddrType::Ssi => 1,
            MacResourceAddrType::EventLabel => 2,
            MacResourceAddrType::Ussi => 3,
            MacResourceAddrType::Smi => 4,
            MacResourceAddrType::SsiAndEventLabel => 5,
            MacResourceAddrType::SsiAndUsageMarker => 6,
            MacResourceAddrType::SmiAndEventLabel => 7,
        }
    }
}

impl From<MacResourceAddrType> for u64 {
    fn from(e: MacResourceAddrType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MacResourceAddrType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MacResourceAddrType::NullPdu => write!(f, "NullPdu"),
            MacResourceAddrType::Ssi => write!(f, "Ssi"),
            MacResourceAddrType::EventLabel => write!(f, "EventLabel"),
            MacResourceAddrType::Ussi => write!(f, "Ussi"),
            MacResourceAddrType::Smi => write!(f, "Smi"),
            MacResourceAddrType::SsiAndEventLabel => write!(f, "SsiAndEventLabel"),
            MacResourceAddrType::SsiAndUsageMarker => write!(f, "SsiAndUsageMarker"),
            MacResourceAddrType::SmiAndEventLabel => write!(f, "SmiAndEventLabel"),
        }
    }
}
