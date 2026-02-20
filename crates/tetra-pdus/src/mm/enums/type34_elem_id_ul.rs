/// Clause 16.10.39 MM PDU types
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MmType34ElemIdUl {
    GroupIdentityLocationDemand = 3,
    GroupReportResponse = 4,
    DmMsAddress = 6,
    GroupIdentityUplink = 8,
    AuthenticationUplink = 9,
    ExtendedCapabilities = 11,
    Proprietary = 15,
}

impl std::convert::TryFrom<u64> for MmType34ElemIdUl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            3 => Ok(MmType34ElemIdUl::GroupIdentityLocationDemand),
            4 => Ok(MmType34ElemIdUl::GroupReportResponse),
            6 => Ok(MmType34ElemIdUl::DmMsAddress),
            8 => Ok(MmType34ElemIdUl::GroupIdentityUplink),
            9 => Ok(MmType34ElemIdUl::AuthenticationUplink),
            11 => Ok(MmType34ElemIdUl::ExtendedCapabilities),
            15 => Ok(MmType34ElemIdUl::Proprietary),
            _ => Err(()),
        }
    }
}

impl MmType34ElemIdUl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MmType34ElemIdUl::GroupIdentityLocationDemand => 3,
            MmType34ElemIdUl::GroupReportResponse => 4,
            MmType34ElemIdUl::DmMsAddress => 6,
            MmType34ElemIdUl::GroupIdentityUplink => 8,
            MmType34ElemIdUl::AuthenticationUplink => 9,
            MmType34ElemIdUl::ExtendedCapabilities => 11,
            MmType34ElemIdUl::Proprietary => 15,
        }
    }
}

impl From<MmType34ElemIdUl> for u64 {
    fn from(e: MmType34ElemIdUl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MmType34ElemIdUl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MmType34ElemIdUl::GroupIdentityLocationDemand => write!(f, "GroupIdentityLocationDemand"),
            MmType34ElemIdUl::GroupReportResponse => write!(f, "GroupReportResponse"),
            MmType34ElemIdUl::DmMsAddress => write!(f, "DmMsAddress"),
            MmType34ElemIdUl::GroupIdentityUplink => write!(f, "GroupIdentityUplink"),
            MmType34ElemIdUl::AuthenticationUplink => write!(f, "AuthenticationUplink"),
            MmType34ElemIdUl::ExtendedCapabilities => write!(f, "ExtendedCapabilities"),
            MmType34ElemIdUl::Proprietary => write!(f, "Proprietary"),
        }
    }
}
