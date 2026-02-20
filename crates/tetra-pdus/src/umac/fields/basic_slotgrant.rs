use core::fmt;

use tetra_core::{BitBuffer, pdu_parse_error::PduParseErr};

use crate::umac::enums::{basic_slotgrant_cap_alloc::BasicSlotgrantCapAlloc, basic_slotgrant_granting_delay::BasicSlotgrantGrantingDelay};

/// 21.5.6 Basic slot granting
#[derive(Debug, Clone)]
pub struct BasicSlotgrant {
    // 4
    pub capacity_allocation: BasicSlotgrantCapAlloc,
    // 4
    pub granting_delay: BasicSlotgrantGrantingDelay,
}

impl BasicSlotgrant {
    pub fn from_bitbuf(buf: &mut BitBuffer) -> Result<Self, PduParseErr> {
        let cap_alloc_val = buf.read_field(4, "capacity_allocation")?;
        let capacity_allocation = BasicSlotgrantCapAlloc::try_from(cap_alloc_val).map_err(|_| PduParseErr::InvalidValue {
            field: "capacity_allocation",
            value: cap_alloc_val,
        })?;

        let granting_delay_val = buf.read_field(4, "granting_delay")?;
        let granting_delay = BasicSlotgrantGrantingDelay::try_from(granting_delay_val).map_err(|_| PduParseErr::InvalidValue {
            field: "granting_delay",
            value: granting_delay_val,
        })?;

        Ok(BasicSlotgrant {
            capacity_allocation,
            granting_delay,
        })
    }

    pub fn to_bitbuf(&self, buf: &mut BitBuffer) {
        buf.write_bits(self.capacity_allocation as u64, 4);
        buf.write_bits(self.granting_delay.into_raw(), 4);
    }
}

impl fmt::Display for BasicSlotgrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BasicSlotgrant {{cap {} delay {} }}",
            self.capacity_allocation, self.granting_delay
        )
    }
}
