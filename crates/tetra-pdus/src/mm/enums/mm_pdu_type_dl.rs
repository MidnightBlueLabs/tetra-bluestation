/// Clause 16.10.39 MM PDU types
/// Bits: 4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MmPduTypeDl {
    DOtar = 0,
    DAuthentication = 1,
    DCkChangeDemand = 2,
    DDisable = 3,
    DEnable = 4,
    DLocationUpdateAccept = 5,
    DLocationUpdateCommand = 6,
    DLocationUpdateReject = 7,
    DLocationUpdateProceeding = 9,
    DAttachDetachGroupIdentity = 10,
    DAttachDetachGroupIdentityAcknowledgement = 11,
    DMmStatus = 12,
    MmPduFunctionNotSupported = 15,
}

impl std::convert::TryFrom<u64> for MmPduTypeDl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(MmPduTypeDl::DOtar),
            1 => Ok(MmPduTypeDl::DAuthentication),
            2 => Ok(MmPduTypeDl::DCkChangeDemand),
            3 => Ok(MmPduTypeDl::DDisable),
            4 => Ok(MmPduTypeDl::DEnable),
            5 => Ok(MmPduTypeDl::DLocationUpdateAccept),
            6 => Ok(MmPduTypeDl::DLocationUpdateCommand),
            7 => Ok(MmPduTypeDl::DLocationUpdateReject),
            9 => Ok(MmPduTypeDl::DLocationUpdateProceeding),
            10 => Ok(MmPduTypeDl::DAttachDetachGroupIdentity),
            11 => Ok(MmPduTypeDl::DAttachDetachGroupIdentityAcknowledgement),
            12 => Ok(MmPduTypeDl::DMmStatus),
            15 => Ok(MmPduTypeDl::MmPduFunctionNotSupported),
            _ => Err(()),
        }
    }
}

impl MmPduTypeDl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MmPduTypeDl::DOtar => 0,
            MmPduTypeDl::DAuthentication => 1,
            MmPduTypeDl::DCkChangeDemand => 2,
            MmPduTypeDl::DDisable => 3,
            MmPduTypeDl::DEnable => 4,
            MmPduTypeDl::DLocationUpdateAccept => 5,
            MmPduTypeDl::DLocationUpdateCommand => 6,
            MmPduTypeDl::DLocationUpdateReject => 7,
            MmPduTypeDl::DLocationUpdateProceeding => 9,
            MmPduTypeDl::DAttachDetachGroupIdentity => 10,
            MmPduTypeDl::DAttachDetachGroupIdentityAcknowledgement => 11,
            MmPduTypeDl::DMmStatus => 12,
            MmPduTypeDl::MmPduFunctionNotSupported => 15,
        }
    }
}

impl From<MmPduTypeDl> for u64 {
    fn from(e: MmPduTypeDl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MmPduTypeDl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MmPduTypeDl::DOtar => write!(f, "DOtar"),
            MmPduTypeDl::DAuthentication => write!(f, "DAuthentication"),
            MmPduTypeDl::DCkChangeDemand => write!(f, "DCkChangeDemand"),
            MmPduTypeDl::DDisable => write!(f, "DDisable"),
            MmPduTypeDl::DEnable => write!(f, "DEnable"),
            MmPduTypeDl::DLocationUpdateAccept => write!(f, "DLocationUpdateAccept"),
            MmPduTypeDl::DLocationUpdateCommand => write!(f, "DLocationUpdateCommand"),
            MmPduTypeDl::DLocationUpdateReject => write!(f, "DLocationUpdateReject"),
            MmPduTypeDl::DLocationUpdateProceeding => write!(f, "DLocationUpdateProceeding"),
            MmPduTypeDl::DAttachDetachGroupIdentity => write!(f, "DAttachDetachGroupIdentity"),
            MmPduTypeDl::DAttachDetachGroupIdentityAcknowledgement => write!(f, "DAttachDetachGroupIdentityAck"),
            MmPduTypeDl::DMmStatus => write!(f, "DMmStatus"),
            MmPduTypeDl::MmPduFunctionNotSupported => write!(f, "MmPduFunctionNotSupported"),
        }
    }
}
