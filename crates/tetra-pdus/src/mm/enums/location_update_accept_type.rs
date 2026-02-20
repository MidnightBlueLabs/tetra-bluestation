/// Clause 16.10.35a Location update accept type
/// Bits: 3
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum LocationUpdateAcceptType {
    RoamingLocationUpdating = 0,
    TemporaryRegistration = 1,
    PeriodicLocationUpdating = 2,
    ItsiAttach = 3,
    ServiceRestorationRoamingLocationUpdating = 4,
    MigratingOrServiceRestorationMigratingLocationUpdating = 5,
    DemandLocationUpdating = 6,
    DisabledMsUpdating = 7,
}

impl std::convert::TryFrom<u64> for LocationUpdateAcceptType {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(LocationUpdateAcceptType::RoamingLocationUpdating),
            1 => Ok(LocationUpdateAcceptType::TemporaryRegistration),
            2 => Ok(LocationUpdateAcceptType::PeriodicLocationUpdating),
            3 => Ok(LocationUpdateAcceptType::ItsiAttach),
            4 => Ok(LocationUpdateAcceptType::ServiceRestorationRoamingLocationUpdating),
            5 => Ok(LocationUpdateAcceptType::MigratingOrServiceRestorationMigratingLocationUpdating),
            6 => Ok(LocationUpdateAcceptType::DemandLocationUpdating),
            7 => Ok(LocationUpdateAcceptType::DisabledMsUpdating),
            _ => Err(()),
        }
    }
}

impl LocationUpdateAcceptType {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            LocationUpdateAcceptType::RoamingLocationUpdating => 0,
            LocationUpdateAcceptType::TemporaryRegistration => 1,
            LocationUpdateAcceptType::PeriodicLocationUpdating => 2,
            LocationUpdateAcceptType::ItsiAttach => 3,
            LocationUpdateAcceptType::ServiceRestorationRoamingLocationUpdating => 4,
            LocationUpdateAcceptType::MigratingOrServiceRestorationMigratingLocationUpdating => 5,
            LocationUpdateAcceptType::DemandLocationUpdating => 6,
            LocationUpdateAcceptType::DisabledMsUpdating => 7,
        }
    }
}

impl From<LocationUpdateAcceptType> for u64 {
    fn from(e: LocationUpdateAcceptType) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for LocationUpdateAcceptType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LocationUpdateAcceptType::RoamingLocationUpdating => write!(f, "RoamingLocationUpdating"),
            LocationUpdateAcceptType::TemporaryRegistration => write!(f, "TemporaryRegistration"),
            LocationUpdateAcceptType::PeriodicLocationUpdating => write!(f, "PeriodicLocationUpdating"),
            LocationUpdateAcceptType::ItsiAttach => write!(f, "ItsiAttach"),
            LocationUpdateAcceptType::ServiceRestorationRoamingLocationUpdating => write!(f, "ServiceRestorationRoamingLocationUpdating"),
            LocationUpdateAcceptType::MigratingOrServiceRestorationMigratingLocationUpdating => {
                write!(f, "MigratingOrServiceRestorationMigratingLocationUpdating")
            }
            LocationUpdateAcceptType::DemandLocationUpdating => write!(f, "DemandLocationUpdating"),
            LocationUpdateAcceptType::DisabledMsUpdating => write!(f, "DisabledMsUpdating"),
        }
    }
}
