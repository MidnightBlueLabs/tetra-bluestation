use crate::cmce::fields::sds_short_report::SdsShortReport;

/// Clause 14.8.34 Pre-coded status
/// The pre-coded status information element shall define general purpose status messages known to all TETRA systems as
/// defined in table 14.72 and shall provide support for the SDS-TL "short reporting" protocol.
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum PreCodedStatus {
    Emergency,
    Reserved(u16),
    SdsTl(SdsShortReport),
    NetworkUserSpecific(u16),
}

impl From<u16> for PreCodedStatus {
    fn from(x: u16) -> Self {
        match x {
            0 => PreCodedStatus::Emergency,
            1..=31742 => PreCodedStatus::Reserved(x),
            31743..=32767 => PreCodedStatus::SdsTl(SdsShortReport::from_u16(x).unwrap()),
            32768..=65535 => PreCodedStatus::NetworkUserSpecific(x),
        }
    }
}

impl PreCodedStatus {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u16 {
        match self {
            PreCodedStatus::Emergency => 0,
            PreCodedStatus::Reserved(x) => x,
            PreCodedStatus::SdsTl(x) => x.to_u16(),
            PreCodedStatus::NetworkUserSpecific(x) => x,
        }
    }
}

impl From<PreCodedStatus> for u16 {
    fn from(e: PreCodedStatus) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for PreCodedStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PreCodedStatus::Emergency => write!(f, "Emergency"),
            PreCodedStatus::Reserved(x) => write!(f, "Reserved({})", x),
            PreCodedStatus::SdsTl(x) => write!(f, "SdsTl({})", x),
            PreCodedStatus::NetworkUserSpecific(x) => write!(f, "NetworkUserSpecific({})", x),
        }
    }
}
