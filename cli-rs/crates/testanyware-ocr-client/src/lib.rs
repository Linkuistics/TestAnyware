//! Host-side client for the TestAnyware EasyOCR Python daemon.
//!
//! Mirrors the Swift code under
//! `cli/Sources/TestAnywareDriver/OCR/`. The daemon wire protocol is
//! line-delimited JSON over stdin/stdout, with image bytes passed via a
//! shared temp PNG path (the daemon reads the file directly):
//!
//!   parent → child : `{"image_path": "/tmp/testanyware-ocr-XXXX.png"}\n`
//!   child  → parent: `{"detections": [...]}\n`  (one line per call)
//!
//! On startup the daemon emits `{"ready": true}\n` once the EasyOCR
//! reader is loaded; the bridge waits for that line before declaring the
//! child usable.
//!
//! The Apple Vision OCR fallback is intentionally *not* ported — it was
//! a 51-line macOS-only fallback, and the Linux primary host target
//! cannot use it. EasyOCR is the canonical engine on every platform.
//!
//! Crate layout:
//!
//! - `bridge`: long-lived child-process actor (`OCRChildBridge`).
//! - `detection`: the `OcrDetection` value type and `OcrResponse`
//!   envelope, wire-compatible with the Swift `OCRDetection` /
//!   `OCRResponse` JSON shape.
//! - `find`: text-finding helpers (case-insensitive substring,
//!   adjacent-word multi-token recovery, deadline-bounded polling).
//! - `status`: persistent degraded-state file written on permanent
//!   failure, cleared on recovery — currently aspirational on the
//!   Swift side; the file is wired here from the start.

pub mod bridge;
pub mod detection;
pub mod find;
pub mod status;

pub use bridge::{OcrBridgeError, OcrChildBridge, OcrChildBridgeConfig};
pub use detection::{OcrDetection, OcrResponse};
pub use find::{find_text, FindOutcome};
pub use status::{OcrStatus, OcrStatusFile};
