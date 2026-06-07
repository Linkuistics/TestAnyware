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
//! Engine selection is per-platform, not one-engine-everywhere (ADR-0002,
//! reversing the earlier "EasyOCR everywhere" call): macOS uses in-process
//! Apple Vision (`vision`, ADR-0003), Linux/Windows use this daemon. See
//! [`engine`] for the `#[cfg]` dispatch seam.
//!
//! Crate layout:
//!
//! - `bridge`: long-lived child-process actor (`OCRChildBridge`).
//! - `engine`: per-platform `OcrEngine` selection + interpreter resolution.
//! - `vision`: in-process Apple Vision engine via `objc2` (macOS only).
//! - `windows_ocr`: in-process native Windows.Media.Ocr engine via the
//!   `windows` WinRT crate (Windows only, ADR-0011).
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
pub mod engine;
pub mod find;
pub mod status;
#[cfg(target_os = "macos")]
mod vision;
#[cfg(windows)]
mod windows_ocr;

pub use bridge::{OcrBridgeError, OcrChildBridge, OcrChildBridgeConfig};
pub use detection::{OcrDetection, OcrResponse};
pub use engine::{resolve_interpreter, OcrEngine};
pub use find::{find_text, FindOutcome};
pub use status::{OcrStatus, OcrStatusFile};
