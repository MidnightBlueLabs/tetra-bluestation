/// 14.8.13 Call status
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CallStatus {
    Callproceeding = 0,
    Callqueued = 1,
    Requestedsubscriberpaged = 2,
    Callcontinue = 3,
    Hangtimeexpired = 4,
}

impl std::convert::TryFrom<u64> for CallStatus {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(CallStatus::Callproceeding),
            1 => Ok(CallStatus::Callqueued),
            2 => Ok(CallStatus::Requestedsubscriberpaged),
            3 => Ok(CallStatus::Callcontinue),
            4 => Ok(CallStatus::Hangtimeexpired),
            _ => Err(()),
        }
    }
}

impl CallStatus {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CallStatus::Callproceeding => 0,
            CallStatus::Callqueued => 1,
            CallStatus::Requestedsubscriberpaged => 2,
            CallStatus::Callcontinue => 3,
            CallStatus::Hangtimeexpired => 4,
        }
    }
}

impl From<CallStatus> for u64 {
    fn from(e: CallStatus) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CallStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CallStatus::Callproceeding => write!(f, "Callproceeding"),
            CallStatus::Callqueued => write!(f, "Callqueued"),
            CallStatus::Requestedsubscriberpaged => write!(f, "Requestedsubscriberpaged"),
            CallStatus::Callcontinue => write!(f, "Callcontinue"),
            CallStatus::Hangtimeexpired => write!(f, "Hangtimeexpired"),
        }
    }
}
