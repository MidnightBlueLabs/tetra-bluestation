/// Clause 29.4.3.11 Short report type
/// The Short report type information element shall indicate the reason for report as defined in table 29.23.
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShortReportType {
    ProtOrEncodingNotSupported = 0,
    DestMemFull = 1,
    MessageReceived = 2,
    MessageConsumed = 3,
}

impl std::convert::TryFrom<u64> for ShortReportType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(ShortReportType::ProtOrEncodingNotSupported),
            1 => Ok(ShortReportType::DestMemFull),
            2 => Ok(ShortReportType::MessageReceived),
            3 => Ok(ShortReportType::MessageConsumed),
            _ => Err(()),
        }
    }
}

impl ShortReportType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            ShortReportType::ProtOrEncodingNotSupported => 0,
            ShortReportType::DestMemFull => 1,
            ShortReportType::MessageReceived => 2,
            ShortReportType::MessageConsumed => 3,
        }
    }
}

impl From<ShortReportType> for u64 {
    fn from(e: ShortReportType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for ShortReportType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ShortReportType::ProtOrEncodingNotSupported => write!(f, "ProtOrEncodingNotSupported"),
            ShortReportType::DestMemFull => write!(f, "DestMemFull"),
            ShortReportType::MessageReceived => write!(f, "MessageReceived"),
            ShortReportType::MessageConsumed => write!(f, "MessageConsumed"),
        }
    }
}
