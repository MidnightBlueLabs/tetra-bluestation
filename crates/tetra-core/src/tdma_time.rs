use core::fmt;

#[derive(Clone, Copy, PartialEq)]
pub struct TdmaTime {
    /// Timeslot, from 1 to 4
    pub t: u8,
    /// Frame number, from 1 to 18
    pub f: u8,
    /// Multiframe number, from 1 to 60
    pub m: u8,
    /// Hyperframe number, from 0 to 0xFFFF
    pub h: u16,
}

impl Default for TdmaTime {
    /// Returns the default TdmaTime of 0/1/1/1
    fn default() -> TdmaTime {
        TdmaTime { h: 0, m: 1, f: 1, t: 1 }
    }
}

/// Value of i32 time where it wraps back to 0.
pub const TIME_INT_WRAP: i32 = 4 * 18 * 60 * 65536;

/// Difference between two int times, handling wrap-around of hyperframe number.
pub fn time_int_diff(a: i32, b: i32) -> i32 {
    let mut diff = a - b;
    while diff < -TIME_INT_WRAP / 2 {
        diff += TIME_INT_WRAP;
    }
    while diff >= TIME_INT_WRAP / 2 {
        diff -= TIME_INT_WRAP;
    }
    diff
}

impl TdmaTime {
    pub fn is_valid(self) -> bool {
        self.t >= 1 && self.t <= 4 && self.f >= 1 && self.f <= 18 && self.m >= 1 && self.m <= 60
    }

    pub fn to_int(self) -> i32 {
        (self.t as i32 - 1) + ((self.f as i32 - 1) * 4) + ((self.m as i32 - 1) * 4 * 18) + (self.h as i32 * 4 * 18 * 60)
    }

    /// Converts a i32 time into a TdmaTime,
    /// truncating the hyperframe number if it exceeds 65535
    pub fn from_int(time: i32) -> TdmaTime {
        let t = (time.rem_euclid(4) + 1) as u8;
        let f = (time.div_euclid(4).rem_euclid(18) + 1) as u8;
        let m = (time.div_euclid(4 * 18).rem_euclid(60) + 1) as u8;
        let h = (time.div_euclid(4 * 18 * 60)) as u16;
        // TODO handle overflow of hyperframe number

        TdmaTime { t, f, m, h }
    }

    /// Add a number of timeslots to a TdmaTime
    pub fn add_timeslots(self, num_slots: i32) -> TdmaTime {
        TdmaTime::from_int(self.to_int() + num_slots)
    }

    /// Difference between two TdmaTimes in timeslots
    pub fn diff(self, b: Self) -> i32 {
        time_int_diff(self.to_int(), b.to_int())
    }

    /// Age of this TdmaTime compared to now
    #[inline(always)]
    pub fn age(self, now: TdmaTime) -> i32 {
        now.diff(self)
    }

    #[inline(always)]
    /// Round this time up to the next occurrence for the given timeslot
    /// If already the right timeslot, time remains unchanged
    pub fn forward_to_timeslot(self, ts: u8) -> TdmaTime {
        let slots_to_add = ((ts + 4 - self.t) % 4) as i32;
        self.add_timeslots(slots_to_add)
    }

    /// Returns true if this DL timeslot should contain a mandatory BSCH (SYNC) block
    pub fn is_mandatory_bsch(&self) -> bool {
        self.f == 18 && self.t == 4 - ((self.m + 1) % 4)
    }

    /// Returns true if this DL timeslot should contain a mandatory BNCH (broadcast) block
    pub fn is_mandatory_bnch(&self) -> bool {
        self.f == 18 && self.t == 4 - ((self.m + 3) % 4)
    }

    /// Returns true if this UL timeslot should contain a mandatory CLCH (Common Linearization) block
    pub fn is_mandatory_clch(&self) -> bool {
        self.f == 18 && self.t == 4 - ((self.m + 1) % 4)
        // self.f == 18 && (self.m + self.t) % 4 == 3
    }
}

impl fmt::Display for TdmaTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:5}/{:02}/{:02}/{}", self.h, self.m, self.f, self.t)
    }
}

impl fmt::Debug for TdmaTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:5}/{:02}/{:02}/{}", self.h, self.m, self.f, self.t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_add_timeslots_and_diff() {
        let initial_time = TdmaTime::default();

        let mut time = initial_time;
        // Repeat add_timeslots enough times that hyperframe number wraps
        let iterations = 100000;
        let increment = 12345;
        for _ in 0..iterations {
            let time2 = time.add_timeslots(increment);
            // Check that difference is computed correctly
            assert_eq!(time2.diff(time), increment);
            assert_eq!(time.diff(time2), -increment);
            time = time2;
        }
        eprintln!("{:?}", time);

        // Go backwards to test that adding a negative number of slots works.
        // It should end up back at initial_time.
        for _ in 0..iterations {
            let time2 = time.add_timeslots(-increment);
            // Check that difference is computed correctly
            assert_eq!(time2.diff(time), -increment);
            assert_eq!(time.diff(time2), increment);
            time = time2;
        }

        assert_eq!(time, initial_time);
    }

    #[test]
    fn test_from_int() {
        // Test both negative and positive numbers
        assert_eq!(TdmaTime::from_int(0), TdmaTime { t: 1, f: 1, m: 1, h: 0 });
        assert_eq!(TdmaTime::from_int(1), TdmaTime { t: 2, f: 1, m: 1, h: 0 });
        assert_eq!(
            TdmaTime::from_int(-1),
            TdmaTime {
                t: 4,
                f: 18,
                m: 60,
                h: 65535
            }
        );
        for time_int in -10000..10000 {
            assert_eq!(TdmaTime::from_int(time_int).diff(TdmaTime::from_int(0)), time_int);
        }
    }
}
