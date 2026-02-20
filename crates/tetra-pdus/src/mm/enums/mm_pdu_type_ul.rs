/// Clause 16.10.39 MM PDU types
/// Bits: 4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MmPduTypeUl {
    UAuthentication = 0,
    UItsiDetach = 1,
    ULocationUpdateDemand = 2,
    UMmStatus = 3,
    UCkChangeResult = 4,
    UOtar = 5,
    UInformationProvide = 6,
    UAttachDetachGroupIdentity = 7,
    UAttachDetachGroupIdentityAcknowledgement = 8,
    UTeiProvide = 9,
    UDisableStatus = 11,
    MmPduFunctionNotSupported = 15,
}

impl std::convert::TryFrom<u64> for MmPduTypeUl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(MmPduTypeUl::UAuthentication),
            1 => Ok(MmPduTypeUl::UItsiDetach),
            2 => Ok(MmPduTypeUl::ULocationUpdateDemand),
            3 => Ok(MmPduTypeUl::UMmStatus),
            4 => Ok(MmPduTypeUl::UCkChangeResult),
            5 => Ok(MmPduTypeUl::UOtar),
            6 => Ok(MmPduTypeUl::UInformationProvide),
            7 => Ok(MmPduTypeUl::UAttachDetachGroupIdentity),
            8 => Ok(MmPduTypeUl::UAttachDetachGroupIdentityAcknowledgement),
            9 => Ok(MmPduTypeUl::UTeiProvide),
            11 => Ok(MmPduTypeUl::UDisableStatus),
            15 => Ok(MmPduTypeUl::MmPduFunctionNotSupported),
            _ => Err(()),
        }
    }
}

impl MmPduTypeUl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MmPduTypeUl::UAuthentication => 0,
            MmPduTypeUl::UItsiDetach => 1,
            MmPduTypeUl::ULocationUpdateDemand => 2,
            MmPduTypeUl::UMmStatus => 3,
            MmPduTypeUl::UCkChangeResult => 4,
            MmPduTypeUl::UOtar => 5,
            MmPduTypeUl::UInformationProvide => 6,
            MmPduTypeUl::UAttachDetachGroupIdentity => 7,
            MmPduTypeUl::UAttachDetachGroupIdentityAcknowledgement => 8,
            MmPduTypeUl::UTeiProvide => 9,
            MmPduTypeUl::UDisableStatus => 11,
            MmPduTypeUl::MmPduFunctionNotSupported => 15,
        }
    }
}

impl From<MmPduTypeUl> for u64 {
    fn from(e: MmPduTypeUl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MmPduTypeUl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MmPduTypeUl::UAuthentication => write!(f, "UAuthentication"),
            MmPduTypeUl::UItsiDetach => write!(f, "UItsiDetach"),
            MmPduTypeUl::ULocationUpdateDemand => write!(f, "ULocationUpdateDemand"),
            MmPduTypeUl::UMmStatus => write!(f, "UMmStatus"),
            MmPduTypeUl::UCkChangeResult => write!(f, "UCkChangeResult"),
            MmPduTypeUl::UOtar => write!(f, "UOtar"),
            MmPduTypeUl::UInformationProvide => write!(f, "UInformationProvide"),
            MmPduTypeUl::UAttachDetachGroupIdentity => write!(f, "UAttachDetachGroupIdentity"),
            MmPduTypeUl::UAttachDetachGroupIdentityAcknowledgement => write!(f, "UAttachDetachGroupIdentityAck"),
            MmPduTypeUl::UTeiProvide => write!(f, "UTeiProvide"),
            MmPduTypeUl::UDisableStatus => write!(f, "UDisableStatus"),
            MmPduTypeUl::MmPduFunctionNotSupported => write!(f, "MmPduFunctionNotSupported"),
        }
    }
}
