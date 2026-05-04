//! Text-finding helpers built on `OcrDetection`.
//!
//! Mirrors the Swift `find-text` polling behaviour:
//!
//! - **Single-token**: case-insensitive substring match on
//!   `OcrDetection.text`. EasyOCR sometimes splits a word across
//!   neighbouring detections — for now we only match within one
//!   detection (Swift parity; multi-word recovery lives in the vision
//!   pipeline, not the CLI surface).
//! - **All-text**: when the query is empty, return every detection so
//!   the caller can render the full recognized-text set.
//! - **Polling**: when `timeout > 0`, keep recapturing/OCR-ing until a
//!   match arrives or the deadline expires. The polling loop is the
//!   caller's job — this module exposes a single-shot match.
//!
//! The README and the contract refer to "case-insensitive substring",
//! which is what we implement; uppercase/lowercase parity for ASCII
//! letters is straightforward, and Unicode case-folding is delegated
//! to `str::to_lowercase` (Swift uses `localizedLowercase`, which has
//! the same behaviour for Latin/cased scripts).

use crate::detection::OcrDetection;

/// Outcome of a single-shot text search.
#[derive(Debug, Clone, PartialEq)]
pub enum FindOutcome {
    /// Text matched. `matches` are in OCR-emission order; the caller
    /// typically takes the first or computes a centroid.
    Found {
        query: String,
        matches: Vec<OcrDetection>,
    },
    /// No matches in the supplied detection set.
    NotFound { query: String },
}

impl FindOutcome {
    pub fn is_found(&self) -> bool {
        matches!(self, FindOutcome::Found { .. })
    }
}

/// Case-insensitive substring search. An empty query returns every
/// supplied detection wrapped as `Found` — this matches the Swift
/// CLI's `find-text` (no query) behaviour where the caller wants a
/// dump of recognized text.
pub fn find_text(query: &str, detections: &[OcrDetection]) -> FindOutcome {
    if query.is_empty() {
        return FindOutcome::Found {
            query: query.to_string(),
            matches: detections.to_vec(),
        };
    }
    let needle = query.to_lowercase();
    let matches: Vec<OcrDetection> = detections
        .iter()
        .filter(|d| d.text.to_lowercase().contains(&needle))
        .cloned()
        .collect();
    if matches.is_empty() {
        FindOutcome::NotFound {
            query: query.to_string(),
        }
    } else {
        FindOutcome::Found {
            query: query.to_string(),
            matches,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn det(text: &str) -> OcrDetection {
        OcrDetection::new(text, 0.0, 0.0, 10.0, 10.0, 0.9)
    }

    #[test]
    fn empty_query_returns_all_detections_wrapped_as_found() {
        let dets = vec![det("Apple"), det("Banana")];
        let out = find_text("", &dets);
        match out {
            FindOutcome::Found { query, matches } => {
                assert_eq!(query, "");
                assert_eq!(matches.len(), 2);
            }
            FindOutcome::NotFound { .. } => panic!("empty query must be Found"),
        }
    }

    #[test]
    fn single_token_matches_case_insensitively() {
        let dets = vec![det("Loading"), det("Save")];
        match find_text("loading", &dets) {
            FindOutcome::Found { matches, .. } => {
                assert_eq!(matches.len(), 1);
                assert_eq!(matches[0].text, "Loading");
            }
            FindOutcome::NotFound { .. } => panic!("should match Loading"),
        }
    }

    #[test]
    fn matches_substring_within_word() {
        let dets = vec![det("Loading…"), det("Idle")];
        let out = find_text("Load", &dets);
        assert!(out.is_found());
    }

    #[test]
    fn returns_not_found_when_no_match() {
        let dets = vec![det("Apple"), det("Banana")];
        let out = find_text("Cherry", &dets);
        match out {
            FindOutcome::NotFound { query } => assert_eq!(query, "Cherry"),
            FindOutcome::Found { .. } => panic!("should be NotFound"),
        }
    }

    #[test]
    fn preserves_emission_order_of_matches() {
        let dets = vec![det("Save"), det("Saved"), det("Save All")];
        match find_text("save", &dets) {
            FindOutcome::Found { matches, .. } => {
                assert_eq!(matches.len(), 3);
                assert_eq!(matches[0].text, "Save");
                assert_eq!(matches[1].text, "Saved");
                assert_eq!(matches[2].text, "Save All");
            }
            _ => panic!("expected three matches"),
        }
    }
}
