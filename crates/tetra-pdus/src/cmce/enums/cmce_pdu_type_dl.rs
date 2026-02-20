/// Clause 14.8.28 PDU type
/// Bits: 5
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CmcePduTypeDl {
    DAlert = 0,
    DCallProceeding = 1,
    DConnect = 2,
    DConnectAcknowledge = 3,
    DDisconnect = 4,
    DInfo = 5,
    DRelease = 6,
    DSetup = 7,
    DStatus = 8,
    DTxCeased = 9,
    DTxContinue = 10,
    DTxGranted = 11,
    DTxWait = 12,
    DTxInterrupt = 13,
    DCallRestore = 14,
    DSdsData = 15,
    DFacility = 16,
    CmceFunctionNotSupported = 31,
}

impl std::convert::TryFrom<u64> for CmcePduTypeDl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(CmcePduTypeDl::DAlert),
            1 => Ok(CmcePduTypeDl::DCallProceeding),
            2 => Ok(CmcePduTypeDl::DConnect),
            3 => Ok(CmcePduTypeDl::DConnectAcknowledge),
            4 => Ok(CmcePduTypeDl::DDisconnect),
            5 => Ok(CmcePduTypeDl::DInfo),
            6 => Ok(CmcePduTypeDl::DRelease),
            7 => Ok(CmcePduTypeDl::DSetup),
            8 => Ok(CmcePduTypeDl::DStatus),
            9 => Ok(CmcePduTypeDl::DTxCeased),
            10 => Ok(CmcePduTypeDl::DTxContinue),
            11 => Ok(CmcePduTypeDl::DTxGranted),
            12 => Ok(CmcePduTypeDl::DTxWait),
            13 => Ok(CmcePduTypeDl::DTxInterrupt),
            14 => Ok(CmcePduTypeDl::DCallRestore),
            15 => Ok(CmcePduTypeDl::DSdsData),
            16 => Ok(CmcePduTypeDl::DFacility),
            31 => Ok(CmcePduTypeDl::CmceFunctionNotSupported),
            _ => Err(()),
        }
    }
}

impl CmcePduTypeDl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            CmcePduTypeDl::DAlert => 0,
            CmcePduTypeDl::DCallProceeding => 1,
            CmcePduTypeDl::DConnect => 2,
            CmcePduTypeDl::DConnectAcknowledge => 3,
            CmcePduTypeDl::DDisconnect => 4,
            CmcePduTypeDl::DInfo => 5,
            CmcePduTypeDl::DRelease => 6,
            CmcePduTypeDl::DSetup => 7,
            CmcePduTypeDl::DStatus => 8,
            CmcePduTypeDl::DTxCeased => 9,
            CmcePduTypeDl::DTxContinue => 10,
            CmcePduTypeDl::DTxGranted => 11,
            CmcePduTypeDl::DTxWait => 12,
            CmcePduTypeDl::DTxInterrupt => 13,
            CmcePduTypeDl::DCallRestore => 14,
            CmcePduTypeDl::DSdsData => 15,
            CmcePduTypeDl::DFacility => 16,
            CmcePduTypeDl::CmceFunctionNotSupported => 31,
        }
    }
}

impl From<CmcePduTypeDl> for u64 {
    fn from(e: CmcePduTypeDl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for CmcePduTypeDl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CmcePduTypeDl::DAlert => write!(f, "DAlert"),
            CmcePduTypeDl::DCallProceeding => write!(f, "DCallProceeding"),
            CmcePduTypeDl::DConnect => write!(f, "DConnect"),
            CmcePduTypeDl::DConnectAcknowledge => write!(f, "DConnectAck"),
            CmcePduTypeDl::DDisconnect => write!(f, "DDisconnect"),
            CmcePduTypeDl::DInfo => write!(f, "DInfo"),
            CmcePduTypeDl::DRelease => write!(f, "DRelease"),
            CmcePduTypeDl::DSetup => write!(f, "DSetup"),
            CmcePduTypeDl::DStatus => write!(f, "DStatus"),
            CmcePduTypeDl::DTxCeased => write!(f, "DTxCeased"),
            CmcePduTypeDl::DTxContinue => write!(f, "DTxContinue"),
            CmcePduTypeDl::DTxGranted => write!(f, "DTxGranted"),
            CmcePduTypeDl::DTxWait => write!(f, "DTxWait"),
            CmcePduTypeDl::DTxInterrupt => write!(f, "DTxInterrupt"),
            CmcePduTypeDl::DCallRestore => write!(f, "DCallRestore"),
            CmcePduTypeDl::DSdsData => write!(f, "DSdsData"),
            CmcePduTypeDl::DFacility => write!(f, "DFacility"),
            CmcePduTypeDl::CmceFunctionNotSupported => write!(f, "CmceFunctionNotSupported"),
        }
    }
}
