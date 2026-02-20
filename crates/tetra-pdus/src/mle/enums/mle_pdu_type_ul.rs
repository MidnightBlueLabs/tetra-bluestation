/// Clause 18.5.20 MLE PDU types
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MlePduTypeUl {
    UPrepare = 0,
    UPrepareDa = 1,
    UIrregularChannelAdvice = 2,
    UChannelClassAdvice = 3,
    URestore = 4,
    UChannelRequest = 6,
    ExtPdu = 7,
}

impl std::convert::TryFrom<u64> for MlePduTypeUl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(MlePduTypeUl::UPrepare),
            1 => Ok(MlePduTypeUl::UPrepareDa),
            2 => Ok(MlePduTypeUl::UIrregularChannelAdvice),
            3 => Ok(MlePduTypeUl::UChannelClassAdvice),
            4 => Ok(MlePduTypeUl::URestore),
            6 => Ok(MlePduTypeUl::UChannelRequest),
            7 => Ok(MlePduTypeUl::ExtPdu),
            _ => Err(()),
        }
    }
}

impl MlePduTypeUl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MlePduTypeUl::UPrepare => 0,
            MlePduTypeUl::UPrepareDa => 1,
            MlePduTypeUl::UIrregularChannelAdvice => 2,
            MlePduTypeUl::UChannelClassAdvice => 3,
            MlePduTypeUl::URestore => 4,
            MlePduTypeUl::UChannelRequest => 6,
            MlePduTypeUl::ExtPdu => 7,
        }
    }
}

impl From<MlePduTypeUl> for u64 {
    fn from(e: MlePduTypeUl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MlePduTypeUl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MlePduTypeUl::UPrepare => write!(f, "UPrepare"),
            MlePduTypeUl::UPrepareDa => write!(f, "UPrepareDa"),
            MlePduTypeUl::UIrregularChannelAdvice => write!(f, "UIrregularChannelAdvice"),
            MlePduTypeUl::UChannelClassAdvice => write!(f, "UChannelClassAdvice"),
            MlePduTypeUl::URestore => write!(f, "URestore"),
            MlePduTypeUl::UChannelRequest => write!(f, "UChannelRequest"),
            MlePduTypeUl::ExtPdu => write!(f, "ExtPdu"),
        }
    }
}
