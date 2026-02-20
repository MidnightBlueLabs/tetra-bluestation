/// Clause 21.4.1 Table 21.38: MAC PDU types for SCH/F, SCH/HD, STCH, SCH-P8/F, SCH-P8/HD, SCH-Q/D, SCH-Q/B and SCH-Q/U
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MacPduType {
    /// TMA-SAP: MAC-RESOURCE (DL) or MAC-DATA (UL)
    MacResourceMacData = 0,
    /// TMA-SAP: MAC-END or MAC-FRAG
    MacFragMacEnd = 1,
    /// TMB-SAP: Broadcast
    Broadcast = 2,
    /// TMA-SAP: Supplementary, or TMD-SAP: MAC-U-SIGNAL
    SuppMacUSignal = 3,
}

impl std::convert::TryFrom<u64> for MacPduType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(MacPduType::MacResourceMacData),
            1 => Ok(MacPduType::MacFragMacEnd),
            2 => Ok(MacPduType::Broadcast),
            3 => Ok(MacPduType::SuppMacUSignal),
            _ => Err(()),
        }
    }
}

impl MacPduType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MacPduType::MacResourceMacData => 0,
            MacPduType::MacFragMacEnd => 1,
            MacPduType::Broadcast => 2,
            MacPduType::SuppMacUSignal => 3,
        }
    }
}

impl From<MacPduType> for u64 {
    fn from(e: MacPduType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MacPduType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MacPduType::MacResourceMacData => write!(f, "MacResourceMacData"),
            MacPduType::MacFragMacEnd => write!(f, "MacFragMacEnd"),
            MacPduType::Broadcast => write!(f, "Broadcast"),
            MacPduType::SuppMacUSignal => write!(f, "SuppMacUSignal"),
        }
    }
}
