/// Clause 18.5.20 MLE PDU types
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MlePduTypeDl {
    DNewCell = 0,
    DPrepareFail = 1,
    DNwrkBroadcast = 2,
    DNwrkBroadcastExt = 3,
    DRestoreAck = 4,
    DRestoreFail = 5,
    DChannelResponse = 6,
    ExtPdu = 7,
}

impl std::convert::TryFrom<u64> for MlePduTypeDl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(MlePduTypeDl::DNewCell),
            1 => Ok(MlePduTypeDl::DPrepareFail),
            2 => Ok(MlePduTypeDl::DNwrkBroadcast),
            3 => Ok(MlePduTypeDl::DNwrkBroadcastExt),
            4 => Ok(MlePduTypeDl::DRestoreAck),
            5 => Ok(MlePduTypeDl::DRestoreFail),
            6 => Ok(MlePduTypeDl::DChannelResponse),
            7 => Ok(MlePduTypeDl::ExtPdu),
            _ => Err(()),
        }
    }
}

impl MlePduTypeDl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MlePduTypeDl::DNewCell => 0,
            MlePduTypeDl::DPrepareFail => 1,
            MlePduTypeDl::DNwrkBroadcast => 2,
            MlePduTypeDl::DNwrkBroadcastExt => 3,
            MlePduTypeDl::DRestoreAck => 4,
            MlePduTypeDl::DRestoreFail => 5,
            MlePduTypeDl::DChannelResponse => 6,
            MlePduTypeDl::ExtPdu => 7,
        }
    }
}

impl From<MlePduTypeDl> for u64 {
    fn from(e: MlePduTypeDl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MlePduTypeDl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MlePduTypeDl::DNewCell => write!(f, "DNewCell"),
            MlePduTypeDl::DPrepareFail => write!(f, "DPrepareFail"),
            MlePduTypeDl::DNwrkBroadcast => write!(f, "DNwrkBroadcast"),
            MlePduTypeDl::DNwrkBroadcastExt => write!(f, "DNwrkBroadcastExt"),
            MlePduTypeDl::DRestoreAck => write!(f, "DRestoreAck"),
            MlePduTypeDl::DRestoreFail => write!(f, "DRestoreFail"),
            MlePduTypeDl::DChannelResponse => write!(f, "DChannelResponse"),
            MlePduTypeDl::ExtPdu => write!(f, "ExtPdu"),
        }
    }
}
