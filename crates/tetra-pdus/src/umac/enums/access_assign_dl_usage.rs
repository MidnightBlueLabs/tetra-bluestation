/// Clause 21.4.7.2 ACCESS-ASSIGN
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessAssignDlUsage {
    Unallocated,
    AssignedControl,
    CommonControl,
    CommonAndAssigned,
    Traffic(u8),
}

impl AccessAssignDlUsage {
    pub fn from_usage_marker(field: u8) -> Self {
        match field {
            0 => AccessAssignDlUsage::Unallocated,
            1 => AccessAssignDlUsage::AssignedControl,
            2 => AccessAssignDlUsage::CommonControl,
            3 => AccessAssignDlUsage::CommonAndAssigned,
            _ => AccessAssignDlUsage::Traffic(field),
        }
    }

    pub fn to_usage_marker(&self) -> u8 {
        match self {
            AccessAssignDlUsage::Unallocated => 0,
            AccessAssignDlUsage::AssignedControl => 1,
            AccessAssignDlUsage::CommonControl => 2,
            AccessAssignDlUsage::CommonAndAssigned => 3,
            AccessAssignDlUsage::Traffic(chan) => *chan,
        }
    }

    pub fn is_traffic(&self) -> bool {
        matches!(self, AccessAssignDlUsage::Traffic(_))
    }

    pub fn get_tchan(&self) -> Option<u8> {
        if let AccessAssignDlUsage::Traffic(chan) = self {
            Some(*chan)
        } else {
            None
        }
    }
}

impl core::fmt::Display for AccessAssignDlUsage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AccessAssignDlUsage::Unallocated => write!(f, "Unallocated"),
            AccessAssignDlUsage::AssignedControl => write!(f, "AssignedControl"),
            AccessAssignDlUsage::CommonControl => write!(f, "CommonControl"),
            AccessAssignDlUsage::CommonAndAssigned => write!(f, "CommonAndAssigned"),
            AccessAssignDlUsage::Traffic(chan) => write!(f, "Traffic({})", chan),
        }
    }
}
