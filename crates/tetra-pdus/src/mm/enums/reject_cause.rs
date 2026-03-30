/// Clause 16.10.42 Reject cause
/// Bits: 5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RejectCause {
    ItsiAtsiUnknown = 1,
    IllegalMs = 2,
    LaNotAllowed = 3,
    LaUnknown = 4,
    NetworkFailure = 5,
    Congestion = 6,
    ForwardRegistrationFailure = 7,
    ServiceNotSubscribed = 8,
    MandatoryElementError = 9,
    MessageConsistencyError = 10,
    RoamingNotSupported = 11,
    MigrationNotSupported = 12,
    NoCipherKsg = 13,
    IdentifiedCipherKsgNotSupported = 14,
    RequestedCipherKeyTypeNotAvailable = 15,
    IdentifiedCipherKeyNotAvailable = 16,
    CipheringRequired = 18,
    AuthenticationFailure = 19,
    UseCaCellNotPermitted = 20,
    UseDaCellNotPermitted = 21,
}

impl std::convert::TryFrom<u64> for RejectCause {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            1 => Ok(RejectCause::ItsiAtsiUnknown),
            2 => Ok(RejectCause::IllegalMs),
            3 => Ok(RejectCause::LaNotAllowed),
            4 => Ok(RejectCause::LaUnknown),
            5 => Ok(RejectCause::NetworkFailure),
            6 => Ok(RejectCause::Congestion),
            7 => Ok(RejectCause::ForwardRegistrationFailure),
            8 => Ok(RejectCause::ServiceNotSubscribed),
            9 => Ok(RejectCause::MandatoryElementError),
            10 => Ok(RejectCause::MessageConsistencyError),
            11 => Ok(RejectCause::RoamingNotSupported),
            12 => Ok(RejectCause::MigrationNotSupported),
            13 => Ok(RejectCause::NoCipherKsg),
            14 => Ok(RejectCause::IdentifiedCipherKsgNotSupported),
            15 => Ok(RejectCause::RequestedCipherKeyTypeNotAvailable),
            16 => Ok(RejectCause::IdentifiedCipherKeyNotAvailable),
            18 => Ok(RejectCause::CipheringRequired),
            19 => Ok(RejectCause::AuthenticationFailure),
            20 => Ok(RejectCause::UseCaCellNotPermitted),
            21 => Ok(RejectCause::UseDaCellNotPermitted),
            _ => Err(()),
        }
    }
}

impl RejectCause {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        self as u64
    }
}

impl From<RejectCause> for u64 {
    fn from(e: RejectCause) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for RejectCause {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RejectCause::ItsiAtsiUnknown => write!(f, "ITSI/ATSI unknown"),
            RejectCause::IllegalMs => write!(f, "Illegal MS"),
            RejectCause::LaNotAllowed => write!(f, "LA not allowed"),
            RejectCause::LaUnknown => write!(f, "LA unknown"),
            RejectCause::NetworkFailure => write!(f, "Network failure"),
            RejectCause::Congestion => write!(f, "Congestion"),
            RejectCause::ForwardRegistrationFailure => write!(f, "Forward registration failure"),
            RejectCause::ServiceNotSubscribed => write!(f, "Service not subscribed"),
            RejectCause::MandatoryElementError => write!(f, "Mandatory element error"),
            RejectCause::MessageConsistencyError => write!(f, "Message consistency error"),
            RejectCause::RoamingNotSupported => write!(f, "Roaming not supported"),
            RejectCause::MigrationNotSupported => write!(f, "Migration not supported"),
            RejectCause::NoCipherKsg => write!(f, "No cipher KSG"),
            RejectCause::IdentifiedCipherKsgNotSupported => write!(f, "Identified cipher KSG not supported"),
            RejectCause::RequestedCipherKeyTypeNotAvailable => write!(f, "Requested cipher key type not available"),
            RejectCause::IdentifiedCipherKeyNotAvailable => write!(f, "Identified cipher key not available"),
            RejectCause::CipheringRequired => write!(f, "Ciphering required"),
            RejectCause::AuthenticationFailure => write!(f, "Authentication failure"),
            RejectCause::UseCaCellNotPermitted => write!(f, "Use of CA cell not permitted"),
            RejectCause::UseDaCellNotPermitted => write!(f, "Use of DA cell not permitted"),
        }
    }
}
