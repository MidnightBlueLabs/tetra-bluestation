/// Clause 16.10.35 Location update type
/// Almost identical to MmLocationUpdateAcceptType
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LocationUpdateType {
    RoamingLocationUpdating = 0,
    TemporaryRegistration = 1,
    PeriodicLocationUpdating = 2,
    ItsiAttach = 3,
    ServiceRestorationRoamingLocationUpdating = 4,
    ServiceRestorationMigratingLocationUpdating = 5,
    DemandLocationUpdating = 6,
    DisabledMsUpdating = 7,
}

impl std::convert::TryFrom<u64> for LocationUpdateType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(LocationUpdateType::RoamingLocationUpdating),
            1 => Ok(LocationUpdateType::TemporaryRegistration),
            2 => Ok(LocationUpdateType::PeriodicLocationUpdating),
            3 => Ok(LocationUpdateType::ItsiAttach),
            4 => Ok(LocationUpdateType::ServiceRestorationRoamingLocationUpdating),
            5 => Ok(LocationUpdateType::ServiceRestorationMigratingLocationUpdating),
            6 => Ok(LocationUpdateType::DemandLocationUpdating),
            7 => Ok(LocationUpdateType::DisabledMsUpdating),
            _ => Err(()),
        }
    }
}

impl LocationUpdateType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            LocationUpdateType::RoamingLocationUpdating => 0,
            LocationUpdateType::TemporaryRegistration => 1,
            LocationUpdateType::PeriodicLocationUpdating => 2,
            LocationUpdateType::ItsiAttach => 3,
            LocationUpdateType::ServiceRestorationRoamingLocationUpdating => 4,
            LocationUpdateType::ServiceRestorationMigratingLocationUpdating => 5,
            LocationUpdateType::DemandLocationUpdating => 6,
            LocationUpdateType::DisabledMsUpdating => 7,
        }
    }
}

impl From<LocationUpdateType> for u64 {
    fn from(e: LocationUpdateType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for LocationUpdateType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LocationUpdateType::RoamingLocationUpdating => write!(f, "RoamingLocationUpdating"),
            LocationUpdateType::TemporaryRegistration => write!(f, "TemporaryRegistration"),
            LocationUpdateType::PeriodicLocationUpdating => write!(f, "PeriodicLocationUpdating"),
            LocationUpdateType::ItsiAttach => write!(f, "ItsiAttach"),
            LocationUpdateType::ServiceRestorationRoamingLocationUpdating => write!(f, "ServiceRestorationRoamingLocationUpdating"),
            LocationUpdateType::ServiceRestorationMigratingLocationUpdating => write!(f, "ServiceRestorationMigratingLocationUpdating"),
            LocationUpdateType::DemandLocationUpdating => write!(f, "DemandLocationUpdating"),
            LocationUpdateType::DisabledMsUpdating => write!(f, "DisabledMsUpdating"),
        }
    }
}
