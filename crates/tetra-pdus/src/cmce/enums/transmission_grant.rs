/// Clause 14.8.42 Transmission grant
/// Bits: 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransmissionGrant {
    Granted = 0,
    NotGranted = 1,
    RequestQueued = 2,
    GrantedToOtherUser = 3,
}

impl std::convert::TryFrom<u64> for TransmissionGrant {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(TransmissionGrant::Granted),
            1 => Ok(TransmissionGrant::NotGranted),
            2 => Ok(TransmissionGrant::RequestQueued),
            3 => Ok(TransmissionGrant::GrantedToOtherUser),
            _ => Err(()),
        }
    }
}

impl TransmissionGrant {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            TransmissionGrant::Granted => 0,
            TransmissionGrant::NotGranted => 1,
            TransmissionGrant::RequestQueued => 2,
            TransmissionGrant::GrantedToOtherUser => 3,
        }
    }
}

impl From<TransmissionGrant> for u64 {
    fn from(e: TransmissionGrant) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for TransmissionGrant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TransmissionGrant::Granted => write!(f, "Granted"),
            TransmissionGrant::NotGranted => write!(f, "NotGranted"),
            TransmissionGrant::RequestQueued => write!(f, "RequestQueued"),
            TransmissionGrant::GrantedToOtherUser => write!(f, "GrantedToOtherUser"),
        }
    }
}
