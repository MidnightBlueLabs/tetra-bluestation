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
            SsiType::EventLabel => write!(f, "EventLabel"),
        }
    }
}

#[derive(Copy, Debug, Clone)]
pub struct TetraAddress {
    pub ssi: u32,
    pub ssi_type: SsiType,
    /// Set to true if the address is an ESI (Encrypted Subscriber Identity)
    /// We maintain this field to allow us to pass still-encrypted SSIs up the stack if we want to
    pub encrypted: bool,
}

impl TetraAddress {
    pub fn new(ssi: u32, ssi_type: SsiType) -> Self {
        Self {
            ssi,
            ssi_type,
            encrypted: false,
        }
    }

    /// Convenience constructor to create ISSI type address
    pub fn issi(ssi: u32) -> Self {
        Self::new(ssi, SsiType::Issi)
    }
}

impl core::fmt::Display for TetraAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.encrypted {
            write!(f, "E_{}:{}", self.ssi_type, self.ssi)
        } else {
            write!(f, "{}:{}", self.ssi_type, self.ssi)
        }
    }
}
