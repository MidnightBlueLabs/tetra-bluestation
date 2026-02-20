/// Clause 14.8.17 Call time-out, set-up phase
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CallTimeoutSetupPhase {
    Predefined = 0,
    T1s = 1,
    T2s = 2,
    T5s = 3,
    T10s = 4,
    T20s = 5,
    T30s = 6,
    T60s = 7,
}

impl std::convert::TryFrom<u64> for CallTimeoutSetupPhase {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(CallTimeoutSetupPhase::Predefined),
            1 => Ok(CallTimeoutSetupPhase::T1s),
            2 => Ok(CallTimeoutSetupPhase::T2s),
            3 => Ok(CallTimeoutSetupPhase::T5s),
            4 => Ok(CallTimeoutSetupPhase::T10s),
            5 => Ok(CallTimeoutSetupPhase::T20s),
            6 => Ok(CallTimeoutSetupPhase::T30s),
            7 => Ok(CallTimeoutSetupPhase::T60s),
            _ => Err(()),
        }
    }
}

impl CallTimeoutSetupPhase {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CallTimeoutSetupPhase::Predefined => 0,
            CallTimeoutSetupPhase::T1s => 1,
            CallTimeoutSetupPhase::T2s => 2,
            CallTimeoutSetupPhase::T5s => 3,
            CallTimeoutSetupPhase::T10s => 4,
            CallTimeoutSetupPhase::T20s => 5,
            CallTimeoutSetupPhase::T30s => 6,
            CallTimeoutSetupPhase::T60s => 7,
        }
    }
}

impl From<CallTimeoutSetupPhase> for u64 {
    fn from(e: CallTimeoutSetupPhase) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CallTimeoutSetupPhase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CallTimeoutSetupPhase::Predefined => write!(f, "Predefined"),
            CallTimeoutSetupPhase::T1s => write!(f, "T1s"),
            CallTimeoutSetupPhase::T2s => write!(f, "T2s"),
            CallTimeoutSetupPhase::T5s => write!(f, "T5s"),
            CallTimeoutSetupPhase::T10s => write!(f, "T10s"),
            CallTimeoutSetupPhase::T20s => write!(f, "T20s"),
            CallTimeoutSetupPhase::T30s => write!(f, "T30s"),
            CallTimeoutSetupPhase::T60s => write!(f, "T60s"),
        }
    }
}
