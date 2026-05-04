//! `OcrDetection` and `OcrResponse` — wire-compatible with the Swift
//! `OCRDetection` / `OCRResponse` JSON shape used by the agent's `/ocr`
//! endpoint and the host-side daemon.

use serde::{Deserialize, Serialize};

/// A single OCR text detection with bounding box in image-pixel
/// coordinates. Matches the Swift `OCRDetection` codable shape.
///
/// `confidence` is `f32` to match the Swift type and the EasyOCR
/// daemon's float32 emission; on the wire it serializes as a JSON
/// number with no precision contract beyond what `serde_json` provides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrDetection {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub confidence: f32,
}

impl OcrDetection {
    pub fn new(
        text: impl Into<String>,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        confidence: f32,
    ) -> Self {
        Self {
            text: text.into(),
            x,
            y,
            width,
            height,
            confidence,
        }
    }

    /// Centre coordinates — what `find-text` returns to scripts that
    /// want a click point, not a bbox.
    pub fn centre(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// `/ocr` server response envelope. `engine` is `"easyocr_daemon"` or
/// `"vision"` (legacy macOS path). `warning` is set when the canonical
/// engine failed and a fallback was used; absent in the success case.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponse {
    pub engine: String,
    pub detections: Vec<OcrDetection>,
    /// Skipped on serialize when `None` so the JSON byte-for-byte matches
    /// Swift's `encodeIfPresent` for the warning key.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub warning: Option<String>,
}

impl OcrResponse {
    pub fn new(engine: impl Into<String>, detections: Vec<OcrDetection>) -> Self {
        Self {
            engine: engine.into(),
            detections,
            warning: None,
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warning = Some(warning.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_round_trips_through_json() {
        let original = OcrDetection::new("Hello", 10.5, 20.0, 100.0, 15.5, 0.95);
        let bytes = serde_json::to_vec(&original).unwrap();
        let decoded: OcrDetection = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn detection_centre_is_midpoint() {
        let d = OcrDetection::new("x", 10.0, 20.0, 100.0, 50.0, 0.9);
        let (cx, cy) = d.centre();
        assert_eq!(cx, 60.0);
        assert_eq!(cy, 45.0);
    }

    #[test]
    fn response_with_nil_warning_omits_the_key() {
        // Swift parity: `OCRResponse.encode` calls `encodeIfPresent` for
        // the warning key, so a nil warning produces JSON without the
        // key at all. Same byte-for-byte requirement on the Rust side
        // for any consumer that diffs serialized output.
        let response = OcrResponse::new("easyocr_daemon", vec![]);
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("warning"), "warning key leaked: {json}");
    }

    #[test]
    fn response_with_warning_emits_the_key() {
        let response =
            OcrResponse::new("vision", vec![]).with_warning("daemon unavailable, using fallback");
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"warning\""));
        assert!(json.contains("daemon unavailable"));
    }

    #[test]
    fn response_round_trips_with_warning() {
        let response = OcrResponse::new(
            "vision",
            vec![OcrDetection::new("Hello", 0.0, 0.0, 50.0, 12.0, 0.99)],
        )
        .with_warning("daemon unavailable, using Vision fallback");
        let bytes = serde_json::to_vec(&response).unwrap();
        let decoded: OcrResponse = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.engine, "vision");
        assert_eq!(decoded.detections.len(), 1);
        assert_eq!(decoded.detections[0].text, "Hello");
        assert_eq!(
            decoded.warning.as_deref(),
            Some("daemon unavailable, using Vision fallback")
        );
    }

    #[test]
    fn response_decodes_from_swift_emitted_shape() {
        // Lock in the wire format the Swift CLI emits today, so a Rust
        // CLI talking to a Swift agent (or vice versa) round-trips.
        // Swift `JSONEncoder` may emit `100` for `100.0` — see memory
        // `swift-jsonencoder-emits-integer-literals-for-whole-number-doubles`.
        let json = r#"{"engine":"easyocr_daemon","detections":[{"text":"Save","x":100,"y":50,"width":40,"height":20,"confidence":0.92}]}"#;
        let response: OcrResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.engine, "easyocr_daemon");
        assert_eq!(response.detections.len(), 1);
        let d = &response.detections[0];
        assert_eq!(d.text, "Save");
        assert_eq!(d.x, 100.0);
        assert_eq!(d.width, 40.0);
        assert!(response.warning.is_none());
    }
}
