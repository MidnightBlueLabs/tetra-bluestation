/// Clause 21.4.7.2 ACCESS-ASSIGN

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessAssignUlUsage {
    CommonOnly,
    CommonAndAssigned,
    AssignedOnly,
    Unallocated,
    Traffic(u8),
}

impl AccessAssignUlUsage {
    pub fn from_usage_marker(field: u8) -> Option<Self> {
        match field {
            0 => Some(AccessAssignUlUsage::Unallocated),
            _ => {
                if field < 4 {
                    tracing::warn!("Invalid usage marker for UL: {}", field);
                    None
                } else {
                    Some(AccessAssignUlUsage::Traffic(field))
                }
            }
        }
    }

    pub fn to_usage_marker(&self) -> Option<u8> {
        match self {
            AccessAssignUlUsage::Unallocated => Some(0),
            AccessAssignUlUsage::Traffic(chan) => Some(*chan),
            _ => None,
        }
    }

    pub fn is_traffic(&self) -> bool {
        matches!(self, AccessAssignUlUsage::Traffic(_))
    }

    pub fn get_tchan(&self) -> Option<u8> {
        if let AccessAssignUlUsage::Traffic(chan) = self {
            Some(*chan)
        } else {
            None
        }
    }
}

impl core::fmt::Display for AccessAssignUlUsage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            AccessAssignUlUsage::CommonOnly => write!(f, "CommonOnly"),
            AccessAssignUlUsage::CommonAndAssigned => write!(f, "CommonAndAssigned"),
            AccessAssignUlUsage::AssignedOnly => write!(f, "AssignedOnly"),
            AccessAssignUlUsage::Traffic(chan) => write!(f, "Traffic({})", chan),
            AccessAssignUlUsage::Unallocated => write!(f, "Unallocated"),
        }
    }
}
