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
    pub fn as_slice(&self) -> &[SsiRange] {
        &self.0
    }
}

/// Takes SsiRanges vec and sorts it by start address, for fast lookups.
/// Also asserts that ranges are disjoint, e.g, do not overlap.
/// Returns a SortedDisjointSsiRanges wrapper which can be used for efficient lookups. See `contains()`.
pub fn sort_disjoint(mut ssi_ranges: Vec<SsiRange>) -> SortedDisjointSsiRanges {
    ssi_ranges.sort_by(|a, b| a.start.cmp(&b.start));

    // Sanity check for overlapping ranges
    let mut lower_bound = 0;
    for range in &ssi_ranges {
        assert!(range.start <= range.end, "Invalid SSI range: {:?}", range);
        assert!(range.start >= lower_bound, "SSI ranges overlap: {:?}", range);
        lower_bound = range.end;
    }

    SortedDisjointSsiRanges(ssi_ranges)
}

/// Takes sorted SsiRanges and checks if the given address falls within any of the ranges.
/// Note that range.end is exclusive, so if an address is exactly equal to range.end, it is not considered local.
pub fn contains(addr: u32, ssi_ranges: &SortedDisjointSsiRanges) -> bool {
    // TODO FIXME this could technically be even faster by starting mid-list and doing binary search
    // Probably fine until we encounter tens of ranges
    for range in ssi_ranges.as_slice() {
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
        let sorted = sort_disjoint(ssi_ranges);
        let s = sorted.as_slice();
        assert_eq!(s[0].start, 100);
        assert_eq!(s[1].start, 300);
        assert_eq!(s[2].start, 500);
    }

    #[test]
    #[should_panic(expected = "SSI ranges overlap")]
    fn test_overlapping_ranges() {
        let ranges = vec![SsiRange { start: 100, end: 200 }, SsiRange { start: 150, end: 300 }];
        sort_disjoint(ranges);
    }

    #[test]
    fn test_adjacent_ranges() {
        let ssi_ranges = vec![SsiRange { start: 100, end: 200 }, SsiRange { start: 200, end: 300 }];
        sort_disjoint(ssi_ranges);
    }

    #[test]
    fn test_containment() {
        let ranges = sort_disjoint(vec![SsiRange { start: 100, end: 200 }, SsiRange { start: 400, end: 500 }]);
        assert!(contains(100, &ranges));
        assert!(contains(150, &ranges));
        assert!(!contains(200, &ranges));
        assert!(!contains(250, &ranges));
        assert!(contains(450, &ranges));
    }

    #[test]
    #[should_panic(expected = "Invalid SSI range")]
    fn test_invalid_range() {
        let ranges = vec![SsiRange { start: 200, end: 100 }];
        sort_disjoint(ranges);
    }
}
