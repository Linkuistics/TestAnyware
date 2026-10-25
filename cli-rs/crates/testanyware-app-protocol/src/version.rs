//! Protocol versions are integers. A side states a range of them.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The protocol range of this release, on both sides. Version 2 adds `GET /v1/slots`, and every
/// request of version 1 keeps its form (spec "admission-queue": "The protocol").
pub const SUPPORTED_RANGE: ProtocolRange = ProtocolRange { min: 1, max: 2 };

/// The first version that has `GET /v1/slots`, which `vm slots` sends.
pub const SLOTS_VERSION: u32 = 2;

/// An inclusive range of protocol versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolRange {
    pub min: u32,
    pub max: u32,
}

impl ProtocolRange {
    pub const fn new(min: u32, max: u32) -> Self {
        Self { min, max }
    }

    /// The highest version both ranges contain, or `None` when they do not overlap.
    pub fn highest_common(self, other: ProtocolRange) -> Option<u32> {
        let high = self.max.min(other.max);
        let low = self.min.max(other.min);
        (low <= high).then_some(high)
    }
}

impl fmt::Display for ProtocolRange {
    /// The form of the protocol header: `1-1`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.min, self.max)
    }
}

/// A protocol range that does not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeParseError(pub String);

impl fmt::Display for RangeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protocol range {:?} is not of the form MIN-MAX", self.0)
    }
}

impl std::error::Error for RangeParseError {}

impl FromStr for ProtocolRange {
    type Err = RangeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bad = || RangeParseError(s.to_string());
        let (min, max) = s.split_once('-').ok_or_else(bad)?;
        let min: u32 = min.trim().parse().map_err(|_| bad())?;
        let max: u32 = max.trim().parse().map_err(|_| bad())?;
        if min > max {
            return Err(bad());
        }
        Ok(Self { min, max })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_ranges_overlap_on_their_version() {
        assert_eq!(SUPPORTED_RANGE.highest_common(SUPPORTED_RANGE), Some(2));
    }

    #[test]
    fn overlapping_ranges_pick_the_highest_common_version() {
        let app = ProtocolRange::new(1, 3);
        let cli = ProtocolRange::new(2, 5);
        assert_eq!(app.highest_common(cli), Some(3));
        assert_eq!(cli.highest_common(app), Some(3));
    }

    #[test]
    fn ranges_that_touch_at_one_version_overlap() {
        assert_eq!(
            ProtocolRange::new(1, 2).highest_common(ProtocolRange::new(2, 4)),
            Some(2)
        );
    }

    #[test]
    fn disjoint_ranges_do_not_overlap() {
        assert_eq!(
            ProtocolRange::new(1, 1).highest_common(ProtocolRange::new(2, 3)),
            None
        );
        assert_eq!(
            ProtocolRange::new(4, 6).highest_common(ProtocolRange::new(1, 3)),
            None
        );
    }

    #[test]
    fn range_round_trips_through_its_header_form() {
        let r = ProtocolRange::new(2, 7);
        assert_eq!(r.to_string(), "2-7");
        assert_eq!("2-7".parse::<ProtocolRange>(), Ok(r));
    }

    #[test]
    fn malformed_ranges_do_not_parse() {
        for bad in ["", "1", "a-b", "3-1", "1-", "-1", "1-2-3"] {
            assert!(bad.parse::<ProtocolRange>().is_err(), "{bad:?} parsed");
        }
    }
}
