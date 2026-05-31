//! In-process Apple **Vision** OCR engine (macOS only).
//!
//! A faithful pure-Rust port of
//! `cli/Sources/TestAnywareDriver/OCR/VisionOCREngine.swift`, using the
//! `objc2` framework bindings (`objc2-vision`, `objc2-core-graphics`,
//! `objc2-core-foundation`, `objc2-foundation`) rather than a Swift shim
//! linked over a C ABI. The FFI choice — and why it is pure-Rust objc2 —
//! is recorded in ADR-0003; it sets the precedent for every other
//! macOS-native facility in the port (e.g. AVAssetWriter for `screen
//! record`).
//!
//! The single entry point [`recognize`] is **synchronous and blocking**:
//! `VNImageRequestHandler::performRequests` runs the recognizer inline.
//! The async `OcrEngine::recognize` wraps it in `spawn_blocking` so the
//! Objective-C objects (none of which are `Send`) live and die on one
//! blocking-pool thread and only the `Send` `Vec<OcrDetection>` crosses
//! back to the async caller.
//!
//! Coordinate space: Vision reports `boundingBox` normalized to `[0, 1]`
//! with the **origin at the image's lower-left corner**. The host
//! contract (matching `screen capture` and the EasyOCR daemon path) is
//! **framebuffer pixels, top-left origin**, so we multiply by the image
//! dimensions and flip Y exactly as the Swift code did:
//! `y = (1 - originY - height) * imageHeight`.

use objc2::runtime::AnyObject;
use objc2::{rc::Retained, AnyThread};
use objc2_core_foundation::CFData;
use objc2_core_graphics::{CGColorRenderingIntent, CGDataProvider, CGImage};
use objc2_foundation::{NSArray, NSDictionary};
use objc2_vision::{
    VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
    VNRequestTextRecognitionLevel,
};

use crate::bridge::OcrBridgeError;
use crate::detection::OcrDetection;

/// Minimum observation confidence to keep a detection. Matches the Swift
/// `VisionOCREngine` threshold (`observation.confidence >= 0.5`).
const MIN_CONFIDENCE: f32 = 0.5;

/// Recognize text in `png` (PNG-encoded bytes) using Apple Vision,
/// returning detections in framebuffer-pixel, top-left-origin
/// coordinates.
///
/// Mirrors `VisionOCREngine.recognize(pngData:)`. A failure to decode the
/// PNG or to schedule the Vision request is surfaced as
/// [`OcrBridgeError::PermanentlyUnavailable`] (the same terminal class the
/// daemon path uses for "this host cannot OCR"); a successful pass that
/// simply finds no text returns an empty `Vec`.
pub(crate) fn recognize(png: &[u8]) -> Result<Vec<OcrDetection>, OcrBridgeError> {
    // PNG bytes → CGImage. Mirrors `CGImage(pngDataProviderSource:)`.
    let data = CFData::from_bytes(png);
    let provider = CGDataProvider::with_cf_data(Some(&data)).ok_or_else(|| {
        OcrBridgeError::PermanentlyUnavailable(
            "Vision OCR: failed to create CGDataProvider from PNG bytes".to_string(),
        )
    })?;
    // SAFETY: `provider` wraps valid PNG bytes; `decode` is null (no
    // remap), interpolation off, default rendering intent — exactly the
    // Swift call's arguments.
    let image = unsafe {
        CGImage::with_png_data_provider(
            Some(&provider),
            std::ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
    .ok_or_else(|| {
        OcrBridgeError::PermanentlyUnavailable(
            "Vision OCR: PNG bytes were not a decodable image".to_string(),
        )
    })?;

    let image_width = CGImage::width(Some(&image)) as f64;
    let image_height = CGImage::height(Some(&image)) as f64;

    // Build the image request handler over the CGImage with empty options.
    let options: Retained<NSDictionary<VNImageOption, AnyObject>> = NSDictionary::new();
    // SAFETY: `image` is a valid CGImage and `options` is an empty,
    // correctly-typed (`VNImageOption` → `AnyObject`) dictionary.
    let handler = unsafe {
        VNImageRequestHandler::initWithCGImage_options(
            VNImageRequestHandler::alloc(),
            &image,
            &options,
        )
    };

    // Configure the recognize-text request: accurate level + language
    // correction, matching the Swift defaults.
    let request = VNRecognizeTextRequest::new();
    request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
    request.setUsesLanguageCorrection(true);

    // performRequests wants an `NSArray<VNRequest>`; `&request` upcasts
    // via the generated superclass `Deref` chain
    // (VNRecognizeTextRequest → VNImageBasedRequest → VNRequest).
    let request_super: &VNRequest = &request;
    let requests = NSArray::from_slice(&[request_super]);

    // A scheduling failure means Vision could not run the request at all
    // — treat as terminal for this host, like the Swift `catch { return [] }`
    // but surfaced as a real error so `--json` reports a code rather than
    // silently empty.
    if let Err(err) = handler.performRequests_error(&requests) {
        return Err(OcrBridgeError::PermanentlyUnavailable(format!(
            "Vision OCR: performRequests failed: {err:?}"
        )));
    }

    // `results()` is `None` when the request produced nothing.
    let Some(observations) = request.results() else {
        return Ok(Vec::new());
    };

    let mut detections = Vec::new();
    for observation in observations.iter() {
        // SAFETY: `confidence`/`boundingBox` are plain property reads on a
        // valid observation returned by the request.
        let observation_confidence = unsafe { observation.confidence() };
        if observation_confidence < MIN_CONFIDENCE {
            continue;
        }
        let candidates = observation.topCandidates(1);
        let Some(candidate) = candidates.iter().next() else {
            continue;
        };

        let text = candidate.string().to_string();
        let confidence = candidate.confidence();
        let bbox = unsafe { observation.boundingBox() };

        // Normalized (lower-left origin) → pixel (top-left origin).
        let x = bbox.origin.x * image_width;
        let y = (1.0 - bbox.origin.y - bbox.size.height) * image_height;
        let width = bbox.size.width * image_width;
        let height = bbox.size.height * image_height;

        detections.push(OcrDetection::new(text, x, y, width, height, confidence));
    }

    Ok(detections)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blank 16×16 white RGBA PNG. Vision rejects images ≤2px per side,
    /// so the test image must clear that floor; 16×16 is comfortably above
    /// it. Vision finds no text in a blank image, so this exercises the
    /// decode → handler → empty-results path without asserting on the
    /// recognizer's output (which belongs to the live-VM gate).
    const BLANK_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x10, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0xf3, 0xff, 0x61, 0x00, 0x00, 0x00, 0x16, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xf8,
        0x4f, 0x21, 0x60, 0x18, 0x35, 0x60, 0xd4, 0x80, 0x51, 0x03, 0x86, 0x8b, 0x01, 0x00, 0x5d,
        0x78, 0xfc, 0x2e, 0xad, 0x21, 0xa9, 0x5e, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
        0xae, 0x42, 0x60, 0x82,
    ];

    #[test]
    fn recognizes_valid_png_without_panicking() {
        let detections = recognize(BLANK_PNG).expect("16x16 PNG should decode and OCR cleanly");
        assert!(
            detections.is_empty(),
            "a blank 16x16 image should yield no text detections"
        );
    }

    #[test]
    fn rejects_non_png_bytes_as_unavailable() {
        let err = recognize(b"definitely not a png").unwrap_err();
        assert_eq!(err.code(), "OCR_UNAVAILABLE");
    }
}
