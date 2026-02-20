/// Clause 21.5.4 Reservation requirement
/// Bits: 4
///
/// Clause 23.5.2.1:
/// If the MS has further signalling to send for this address on this control channel, the MS-MAC shall include the
/// "reservation requirement" element whenever it transmits an SCH MAC block (i.e. SCH/HU, SCH-P8/HU, SCH-Q/HU,
/// SCH-Q/RA, SCH/F, SCH-P8/F or SCH-Q/U) containing a MAC-ACCESS, MAC-DATA, MAC-U-BLCK,
/// MAC-END-HU or MAC-END PDU. If PDU association is used within the MAC block then the "reservation
/// requirement" element shall be included in the last (non-null) PDU in the MAC block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ReservationRequirement {
    Req1Subslot = 0,
    Req1Slot = 1,
    Req2Slots = 2,
    Req3Slots = 3,
    Req4Slots = 4,
    Req5Slots = 5,
    Req6Slots = 6,
    Req8Slots = 7,
    Req10Slots = 8,
    Req13Slots = 9,
    Req17Slots = 10,
    Req24Slots = 11,
    Req34Slots = 12,
    Req51Slots = 13,
    Req68Slots = 14,
    /// Value 15, or, when encountered in MAC-U-BLCK, value 14
    ReqOver68 = 15,
    // Value 15 when encountered in MAC-U-BLCK.
    // ReqNone,
}

impl std::convert::TryFrom<u64> for ReservationRequirement {
    type Error = ();
    fn try_from(x: u64) -> Result<Self, Self::Error> {
        match x {
            0 => Ok(ReservationRequirement::Req1Subslot),
            1 => Ok(ReservationRequirement::Req1Slot),
            2 => Ok(ReservationRequirement::Req2Slots),
            3 => Ok(ReservationRequirement::Req3Slots),
            4 => Ok(ReservationRequirement::Req4Slots),
            5 => Ok(ReservationRequirement::Req5Slots),
            6 => Ok(ReservationRequirement::Req6Slots),
            7 => Ok(ReservationRequirement::Req8Slots),
            8 => Ok(ReservationRequirement::Req10Slots),
            9 => Ok(ReservationRequirement::Req13Slots),
            10 => Ok(ReservationRequirement::Req17Slots),
            11 => Ok(ReservationRequirement::Req24Slots),
            12 => Ok(ReservationRequirement::Req34Slots),
            13 => Ok(ReservationRequirement::Req51Slots),
            14 => Ok(ReservationRequirement::Req68Slots),
            15 => Ok(ReservationRequirement::ReqOver68),
            _ => Err(()),
        }
    }
}

impl ReservationRequirement {
    /// Convert this enum back into the raw integer value
    pub fn into_raw(self) -> u64 {
        match self {
            ReservationRequirement::Req1Subslot => 0,
            ReservationRequirement::Req1Slot => 1,
            ReservationRequirement::Req2Slots => 2,
            ReservationRequirement::Req3Slots => 3,
            ReservationRequirement::Req4Slots => 4,
            ReservationRequirement::Req5Slots => 5,
            ReservationRequirement::Req6Slots => 6,
            ReservationRequirement::Req8Slots => 7,
            ReservationRequirement::Req10Slots => 8,
            ReservationRequirement::Req13Slots => 9,
            ReservationRequirement::Req17Slots => 10,
            ReservationRequirement::Req24Slots => 11,
            ReservationRequirement::Req34Slots => 12,
            ReservationRequirement::Req51Slots => 13,
            ReservationRequirement::Req68Slots => 14,
            ReservationRequirement::ReqOver68 => 15,
        }
    }

    /// Pass 0 when just a single subslot is required
    pub fn from_req_slotcount(req: usize) -> Self {
        match req {
            0 => ReservationRequirement::Req1Subslot,
            1 => ReservationRequirement::Req1Slot,
            2 => ReservationRequirement::Req2Slots,
            3 => ReservationRequirement::Req3Slots,
            4 => ReservationRequirement::Req4Slots,
            5 => ReservationRequirement::Req5Slots,
            6 => ReservationRequirement::Req6Slots,
            7..=8 => ReservationRequirement::Req8Slots,
            9..=10 => ReservationRequirement::Req10Slots,
            11..=13 => ReservationRequirement::Req13Slots,
            14..=17 => ReservationRequirement::Req17Slots,
            18..=24 => ReservationRequirement::Req24Slots,
            25..=34 => ReservationRequirement::Req34Slots,
            35..=51 => ReservationRequirement::Req51Slots,
            52..=68 => ReservationRequirement::Req68Slots,
            _ => ReservationRequirement::ReqOver68,
        }
    }

    /// Returns 0 when just a single subslot is required
    /// Returns 99 when over 69 subslots are required
    pub fn to_req_slotcount(&self) -> usize {
        match self {
            ReservationRequirement::Req1Subslot => {
                unimplemented!();
                // 0
            }
            ReservationRequirement::Req1Slot => 1,
            ReservationRequirement::Req2Slots => 2,
            ReservationRequirement::Req3Slots => 3,
            ReservationRequirement::Req4Slots => 4,
            ReservationRequirement::Req5Slots => 5,
            ReservationRequirement::Req6Slots => 6,
            ReservationRequirement::Req8Slots => 8,
            ReservationRequirement::Req10Slots => 10,
            ReservationRequirement::Req13Slots => 13,
            ReservationRequirement::Req17Slots => 17,
            ReservationRequirement::Req24Slots => 24,
            ReservationRequirement::Req34Slots => 34,
            ReservationRequirement::Req51Slots => 51,
            ReservationRequirement::Req68Slots => 68,
            ReservationRequirement::ReqOver68 => {
                unimplemented!();
                // 99
            }
        }
    }
}

impl From<ReservationRequirement> for u64 {
    fn from(e: ReservationRequirement) -> Self {
        e.into_raw()
    }
}

impl core::fmt::Display for ReservationRequirement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ReservationRequirement::Req1Subslot => write!(f, "Req1Subslot"),
            ReservationRequirement::Req1Slot => write!(f, "Req1Slot"),
            ReservationRequirement::Req2Slots => write!(f, "Req2Slots"),
            ReservationRequirement::Req3Slots => write!(f, "Req3Slots"),
            ReservationRequirement::Req4Slots => write!(f, "Req4Slots"),
            ReservationRequirement::Req5Slots => write!(f, "Req5Slots"),
            ReservationRequirement::Req6Slots => write!(f, "Req6Slots"),
            ReservationRequirement::Req8Slots => write!(f, "Req8Slots"),
            ReservationRequirement::Req10Slots => write!(f, "Req10Slots"),
            ReservationRequirement::Req13Slots => write!(f, "Req13Slots"),
            ReservationRequirement::Req17Slots => write!(f, "Req17Slots"),
            ReservationRequirement::Req24Slots => write!(f, "Req24Slots"),
            ReservationRequirement::Req34Slots => write!(f, "Req34Slots"),
            ReservationRequirement::Req51Slots => write!(f, "Req51Slots"),
            ReservationRequirement::Req68Slots => write!(f, "Req68Slots"),
            ReservationRequirement::ReqOver68 => write!(f, "ReqOver68"),
        }
    }
}
