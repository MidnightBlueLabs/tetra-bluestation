#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeslotOwner {
    Brew,
    Cmce,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeslotAllocErr {
    InvalidTimeslot(u8),
    InUse {
        ts: u8,
        owner: TimeslotOwner,
    },
    NotAllocated {
        ts: u8,
    },
    OwnerMismatch {
        ts: u8,
        owner: TimeslotOwner,
        actual: TimeslotOwner,
    },
}

#[derive(Debug, Clone)]
pub struct TimeslotAllocator {
    // Index 0 = TS2, 1 = TS3, 2 = TS4
    owners: [Option<TimeslotOwner>; 3],
}

impl Default for TimeslotAllocator {
    fn default() -> Self {
        Self {
            owners: [None, None, None],
        }
    }
}

impl TimeslotAllocator {
    fn idx(ts: u8) -> Result<usize, TimeslotAllocErr> {
        if (2..=4).contains(&ts) {
            Ok((ts - 2) as usize)
        } else {
            Err(TimeslotAllocErr::InvalidTimeslot(ts))
        }
    }

    pub fn allocate_any(&mut self, owner: TimeslotOwner) -> Option<u8> {
        for (i, slot) in self.owners.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(owner);
                return Some(i as u8 + 2);
            }
        }
        None
    }

    pub fn reserve(&mut self, owner: TimeslotOwner, ts: u8) -> Result<(), TimeslotAllocErr> {
        let idx = Self::idx(ts)?;
        match self.owners[idx] {
            None => {
                self.owners[idx] = Some(owner);
                Ok(())
            }
            Some(existing) => Err(TimeslotAllocErr::InUse { ts, owner: existing }),
        }
    }

    pub fn release(&mut self, owner: TimeslotOwner, ts: u8) -> Result<(), TimeslotAllocErr> {
        let idx = Self::idx(ts)?;
        match self.owners[idx] {
            None => Err(TimeslotAllocErr::NotAllocated { ts }),
            Some(existing) if existing != owner => Err(TimeslotAllocErr::OwnerMismatch {
                ts,
                owner,
                actual: existing,
            }),
            Some(_) => {
                self.owners[idx] = None;
                Ok(())
            }
        }
    }

    pub fn owner(&self, ts: u8) -> Option<TimeslotOwner> {
        Self::idx(ts).ok().and_then(|idx| self.owners[idx])
    }

    pub fn is_free(&self, ts: u8) -> bool {
        self.owner(ts).is_none()
    }
}
