/// Clause 14.8.5 / 14.8.9 — Called/Calling Party Type Identifier (CPTI).
/// Indicates the type of address which follows in the PDU (Table 14.39).
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PartyTypeIdentifier {
    /// Short Number Address (SNA)
    Sna = 0,
    /// Short Subscriber Identity (SSI)
    Ssi = 1,
    /// TETRA Subscriber Identity (TSI = SSI + Extension)
    Tsi = 2,
    /// Reserved
    Reserved = 3,
}

impl std::convert::TryFrom<u64> for PartyTypeIdentifier {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(PartyTypeIdentifier::Sna),
            1 => Ok(PartyTypeIdentifier::Ssi),
            2 => Ok(PartyTypeIdentifier::Tsi),
            3 => Ok(PartyTypeIdentifier::Reserved),
            _ => Err(()),
        }
    }
}

impl PartyTypeIdentifier {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        self as u64
    }
}

impl From<PartyTypeIdentifier> for u64 {
    fn from(e: PartyTypeIdentifier) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for PartyTypeIdentifier {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PartyTypeIdentifier::Sna => write!(f, "SNA"),
            PartyTypeIdentifier::Ssi => write!(f, "SSI"),
            PartyTypeIdentifier::Tsi => write!(f, "TSI"),
            PartyTypeIdentifier::Reserved => write!(f, "Reserved"),
        }
    }
}
