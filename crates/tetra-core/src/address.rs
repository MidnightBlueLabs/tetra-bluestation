#[allow(dead_code)]
#[derive(Copy, Debug, Clone, PartialEq)]
pub enum SsiType {
    Unknown,
    /// Generic type when specific type unknown. Avoid using where possible.
    Ssi,
    /// Individual Short Subscriber Identity
    Issi,
    /// Group Short Subscriber Identity
    Gssi,
    Ussi,
    Smi,

    /// Any type of encrypted SSI
    Esi,

    /// Only usable in Umac, needs to be replaced with true SSI
    EventLabel,
}

impl core::fmt::Display for SsiType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SsiType::Unknown => write!(f, "Unknown"),
            SsiType::Ssi => write!(f, "SSI"),
            SsiType::Issi => write!(f, "ISSI"),
            SsiType::Gssi => write!(f, "GSSI"),
            SsiType::Ussi => write!(f, "USSI"),
            SsiType::Smi => write!(f, "SMI"),
            SsiType::Esi => write!(f, "ESI"),
            SsiType::EventLabel => write!(f, "EventLabel"),
        }
    }
}

#[derive(Copy, Debug, Clone)]
pub struct TetraAddress {
    pub ssi: u32,
    pub ssi_type: SsiType,
}

impl TetraAddress {
    pub fn new(ssi: u32, ssi_type: SsiType) -> Self {
        Self { ssi, ssi_type }
    }

    /// Convenience constructor to create ISSI type address
    pub fn issi(ssi: u32) -> Self {
        Self::new(ssi, SsiType::Issi)
    }
}

impl core::fmt::Display for TetraAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}:{}", self.ssi_type, self.ssi)
    }
}
