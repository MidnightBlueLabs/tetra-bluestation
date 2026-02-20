/// Clause 14.8.28 PDU type
/// Bits: 5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CmcePduTypeUl {
    UAlert = 0,
    UConnect = 2,
    UDisconnect = 4,
    UInfo = 5,
    URelease = 6,
    USetup = 7,
    UStatus = 8,
    UTxCeased = 9,
    UTxDemand = 10,
    UCallRestore = 14,
    USdsData = 15,
    UFacility = 16,
    CmceFunctionNotSupported = 31,
}

impl std::convert::TryFrom<u64> for CmcePduTypeUl {
    type Error = ();
    fn try_from(raw: u64) -> Result<Self, Self::Error> {
        let x = raw as u8;
        match x {
            0 => Ok(CmcePduTypeUl::UAlert),
            2 => Ok(CmcePduTypeUl::UConnect),
            4 => Ok(CmcePduTypeUl::UDisconnect),
            5 => Ok(CmcePduTypeUl::UInfo),
            6 => Ok(CmcePduTypeUl::URelease),
            7 => Ok(CmcePduTypeUl::USetup),
            8 => Ok(CmcePduTypeUl::UStatus),
            9 => Ok(CmcePduTypeUl::UTxCeased),
            10 => Ok(CmcePduTypeUl::UTxDemand),
            14 => Ok(CmcePduTypeUl::UCallRestore),
            15 => Ok(CmcePduTypeUl::USdsData),
            16 => Ok(CmcePduTypeUl::UFacility),
            31 => Ok(CmcePduTypeUl::CmceFunctionNotSupported),
            _ => Err(()),
        }
    }
}

impl CmcePduTypeUl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CmcePduTypeUl::UAlert => 0,
            CmcePduTypeUl::UConnect => 2,
            CmcePduTypeUl::UDisconnect => 4,
            CmcePduTypeUl::UInfo => 5,
            CmcePduTypeUl::URelease => 6,
            CmcePduTypeUl::USetup => 7,
            CmcePduTypeUl::UStatus => 8,
            CmcePduTypeUl::UTxCeased => 9,
            CmcePduTypeUl::UTxDemand => 10,
            CmcePduTypeUl::UCallRestore => 14,
            CmcePduTypeUl::USdsData => 15,
            CmcePduTypeUl::UFacility => 16,
            CmcePduTypeUl::CmceFunctionNotSupported => 31,
        }
    }
}

impl From<CmcePduTypeUl> for u64 {
    fn from(e: CmcePduTypeUl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CmcePduTypeUl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CmcePduTypeUl::UAlert => write!(f, "UAlert"),
            CmcePduTypeUl::UConnect => write!(f, "UConnect"),
            CmcePduTypeUl::UDisconnect => write!(f, "UDisconnect"),
            CmcePduTypeUl::UInfo => write!(f, "UInfo"),
            CmcePduTypeUl::URelease => write!(f, "URelease"),
            CmcePduTypeUl::USetup => write!(f, "USetup"),
            CmcePduTypeUl::UStatus => write!(f, "UStatus"),
            CmcePduTypeUl::UTxCeased => write!(f, "UTxCeased"),
            CmcePduTypeUl::UTxDemand => write!(f, "UTxDemand"),
            CmcePduTypeUl::UCallRestore => write!(f, "UCallRestore"),
            CmcePduTypeUl::USdsData => write!(f, "USdsData"),
            CmcePduTypeUl::UFacility => write!(f, "UFacility"),
            CmcePduTypeUl::CmceFunctionNotSupported => write!(f, "CmceFunctionNotSupported"),
        }
    }
}
