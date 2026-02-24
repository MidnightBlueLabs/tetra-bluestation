#[derive(Debug, Clone)]
pub struct SsiRange {
    /// Inclusive start of the range
    pub start: u32,
    /// Exclusive end of the range. E.g. if end is 200, the last valid address in the range is 199.
    pub end: u32,
}

/// A sorted, non-overlapping (disjoint) list of SSI ranges.
/// Can only be constructed via `sort_non_overlapping()`.
#[derive(Debug, Clone)]
pub struct SortedDisjointSsiRanges(Vec<SsiRange>);
impl SortedDisjointSsiRanges {
    /// Takes Vec<SsiRange> and sorts it by start address, for fast lookups.
    /// Also asserts that ranges are disjoint, e.g, do not overlap.
    /// Returns a SortedDisjointSsiRanges wrapper which can be used for efficient lookups. See `contains()`.
    pub fn from_vec_ssirange(mut ranges: Vec<SsiRange>) -> Self {
        ranges.sort_by(|a, b| a.start.cmp(&b.start));

        // Sanity check for overlapping ranges
        let mut lower_bound = 0;
        for range in &ranges {
            assert!(range.start <= range.end, "Invalid SSI range: {:?}", range);
            assert!(range.start >= lower_bound, "SSI ranges overlap: {:?}", range);
            lower_bound = range.end;
        }
        Self(ranges)
    }

    /// Takes Vec<(start: u32, end: u32)> and sorts it by start address, for fast lookups.
    /// Also asserts that ranges are disjoint, e.g, do not overlap.
    /// Returns a SortedDisjointSsiRanges wrapper which can be used for efficient lookups. See `contains()`.
    pub fn from_vec_tuple(tuples: Vec<(u32, u32)>) -> Self {
        let ssi_ranges = tuples.into_iter().map(|(start, end)| SsiRange { start, end }).collect();
        Self::from_vec_ssirange(ssi_ranges)
    }

    pub fn as_slice(&self) -> &[SsiRange] {
        &self.0
    }

    /// Checks if the given address falls within any of the ranges.
    /// Note that range.end is exclusive, so if an address is exactly equal to range.end, it is not considered local.
    pub fn contains(&self, addr: u32) -> bool {
        // TODO FIXME this could technically be even faster by starting mid-list and doing binary search
        // Probably fine until we encounter tens of ranges
        for range in self.as_slice() {
            if addr >= range.start && addr < range.end {
                return true;
            }
            if range.end > addr {
                // Since ranges are sorted, we can stop checking once we've passed the address
                break;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_sorting() {
        let ssi_ranges = vec![
            SsiRange { start: 300, end: 400 },
            SsiRange { start: 100, end: 200 },
            SsiRange { start: 500, end: 600 },
        ];
        let sorted = SortedDisjointSsiRanges::from_vec_ssirange(ssi_ranges);
        let s = sorted.as_slice();
        assert_eq!(s[0].start, 100);
        assert_eq!(s[1].start, 300);
        assert_eq!(s[2].start, 500);
    }

    #[test]
    #[should_panic(expected = "SSI ranges overlap")]
    fn test_overlapping_ranges() {
        let ranges = vec![SsiRange { start: 100, end: 200 }, SsiRange { start: 150, end: 300 }];
        SortedDisjointSsiRanges::from_vec_ssirange(ranges);
    }

    #[test]
    fn test_adjacent_ranges() {
        let ssi_ranges = vec![SsiRange { start: 100, end: 200 }, SsiRange { start: 200, end: 300 }];
        SortedDisjointSsiRanges::from_vec_ssirange(ssi_ranges);
    }

    #[test]
    fn test_containment() {
        let ranges = SortedDisjointSsiRanges::from_vec_ssirange(vec![SsiRange { start: 100, end: 200 }, SsiRange { start: 400, end: 500 }]);
        assert!(ranges.contains(100));
        assert!(ranges.contains(150));
        assert!(!ranges.contains(200));
        assert!(!ranges.contains(250));
        assert!(ranges.contains(450));
    }

    #[test]
    #[should_panic(expected = "Invalid SSI range")]
    fn test_invalid_range() {
        let ranges = vec![SsiRange { start: 200, end: 100 }];
        SortedDisjointSsiRanges::from_vec_ssirange(ranges);
    }
}
