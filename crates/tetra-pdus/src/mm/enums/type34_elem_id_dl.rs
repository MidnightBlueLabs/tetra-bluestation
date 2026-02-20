/// Clause 16.10.51 Type 3/4 element identifier
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MmType34ElemIdDl {
    DefaultGroupAttachLifetime = 1,
    NewRegisteredArea = 2,
    SecurityDownlink = 3,
    GroupReportResponse = 4,
    GroupIdentityLocationAccept = 5,
    DmMsAddress = 6,
    GroupIdentityDownlink = 7,
    AuthenticationDownlink = 10,
    GroupIdentitySecurityRelatedInformation = 12,
    CellTypeControl = 13,
    Proprietary = 15,
}

impl std::convert::TryFrom<u64> for MmType34ElemIdDl {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            1 => Ok(MmType34ElemIdDl::DefaultGroupAttachLifetime),
            2 => Ok(MmType34ElemIdDl::NewRegisteredArea),
            3 => Ok(MmType34ElemIdDl::SecurityDownlink),
            4 => Ok(MmType34ElemIdDl::GroupReportResponse),
            5 => Ok(MmType34ElemIdDl::GroupIdentityLocationAccept),
            6 => Ok(MmType34ElemIdDl::DmMsAddress),
            7 => Ok(MmType34ElemIdDl::GroupIdentityDownlink),
            10 => Ok(MmType34ElemIdDl::AuthenticationDownlink),
            12 => Ok(MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation),
            13 => Ok(MmType34ElemIdDl::CellTypeControl),
            15 => Ok(MmType34ElemIdDl::Proprietary),
            _ => Err(()),
        }
    }
}

impl MmType34ElemIdDl {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            MmType34ElemIdDl::DefaultGroupAttachLifetime => 1,
            MmType34ElemIdDl::NewRegisteredArea => 2,
            MmType34ElemIdDl::SecurityDownlink => 3,
            MmType34ElemIdDl::GroupReportResponse => 4,
            MmType34ElemIdDl::GroupIdentityLocationAccept => 5,
            MmType34ElemIdDl::DmMsAddress => 6,
            MmType34ElemIdDl::GroupIdentityDownlink => 7,
            MmType34ElemIdDl::AuthenticationDownlink => 10,
            MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation => 12,
            MmType34ElemIdDl::CellTypeControl => 13,
            MmType34ElemIdDl::Proprietary => 15,
        }
    }
}

impl From<MmType34ElemIdDl> for u64 {
    fn from(e: MmType34ElemIdDl) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for MmType34ElemIdDl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MmType34ElemIdDl::DefaultGroupAttachLifetime => write!(f, "DefaultGroupAttachLifetime"),
            MmType34ElemIdDl::NewRegisteredArea => write!(f, "NewRegisteredArea"),
            MmType34ElemIdDl::SecurityDownlink => write!(f, "SecurityDownlink"),
            MmType34ElemIdDl::GroupReportResponse => write!(f, "GroupReportResponse"),
            MmType34ElemIdDl::GroupIdentityLocationAccept => write!(f, "GroupIdentityLocationAccept"),
            MmType34ElemIdDl::DmMsAddress => write!(f, "DmMsAddress"),
            MmType34ElemIdDl::GroupIdentityDownlink => write!(f, "GroupIdentityDownlink"),
            MmType34ElemIdDl::AuthenticationDownlink => write!(f, "AuthenticationDownlink"),
            MmType34ElemIdDl::GroupIdentitySecurityRelatedInformation => write!(f, "GroupIdentitySecurityRelatedInformation"),
            MmType34ElemIdDl::CellTypeControl => write!(f, "CellTypeControl"),
            MmType34ElemIdDl::Proprietary => write!(f, "Proprietary"),
        }
    }
}
