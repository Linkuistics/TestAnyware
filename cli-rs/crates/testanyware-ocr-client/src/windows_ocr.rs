//! In-process native **Windows.Media.Ocr** engine (Windows only).
//!
//! The Windows arm of the per-platform `OcrEngine` seam (ADR-0002), decided
//! in ADR-0011: the host calls the built-in WinRT OCR API directly, with no
//! Python daemon and no end-user runtime dependency. Bound through the
//! Microsoft-official pure-Rust `windows` crate (WinRT) rather than a C#/.NET
//! shim — the direct analogue of the `objc2`-over-Swift-shim choice ADR-0003
//! made for the macOS Vision engine ([`crate::vision`]), recurring for WinRT.
//!
//! The single entry point [`recognize`] is **synchronous and blocking**: it
//! drives the WinRT `IAsyncOperation`s to completion in-thread via `.join()`.
//! The async `OcrEngine::recognize` wraps it in `spawn_blocking` so the
//! non-`Send` WinRT objects live and die on one blocking-pool thread and only
//! the `Send` `Vec<OcrDetection>` crosses back to the async caller — the same
//! shape as `vision.rs`.
//!
//! ## Coordinate space
//! WinRT `OcrWord::BoundingRect` is already in **image pixels with a top-left
//! origin** — the same space as `screen capture` and the host contract — so,
//! unlike Vision's lower-left *normalized* boxes, there is **no Y-flip** and no
//! scaling. The rect maps straight onto [`OcrDetection`].
//!
//! ## Detection granularity: per **word**
//! `Windows.Media.Ocr` returns lines, each holding words; only `OcrWord`
//! carries a `BoundingRect` (an `OcrLine` exposes text but no box). We emit
//! **one detection per word**, which is also what `find-text` consumers want:
//! a tight, clickable box around the matched token (e.g. the "File" menu),
//! rather than a line box spanning the whole menu bar whose centre would miss
//! the target. This matches the CLI's "substring match within a single
//! detection" contract ([`crate::find`]) and the word/phrase granularity of
//! both the EasyOCR daemon and the macOS Vision arm.
//!
//! ## Confidence
//! `Windows.Media.Ocr` exposes **no per-word confidence**, so we synthesize a
//! fixed [`SYNTHETIC_CONFIDENCE`] (`1.0`) — recorded in ADR-0011's
//! consequences. Unlike Vision (which drops observations below 0.5), there is
//! no threshold to apply: every recognized word is kept.

use windows::core::HSTRING;
use windows::Globalization::Language;
use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat};
use windows::Media::Ocr::OcrEngine as WinOcrEngine;
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

use crate::bridge::OcrBridgeError;
use crate::detection::OcrDetection;

/// Confidence stamped on every detection: `Windows.Media.Ocr` reports none
/// per word (ADR-0011). `1.0` so consumers that sort/threshold on confidence
/// treat all Windows detections as fully trusted.
const SYNTHETIC_CONFIDENCE: f32 = 1.0;

/// BCP-47 tag used when the user profile carries no OCR-capable language.
/// English OCR ships by default on the Windows images this targets, so this
/// fallback is belt-and-braces, not the common path.
const FALLBACK_LANGUAGE: &str = "en-US";

/// Recognize text in `png` (PNG-encoded bytes) using the native
/// Windows.Media.Ocr engine, returning **per-word** detections in
/// framebuffer-pixel, top-left-origin coordinates.
///
/// Any decode / engine-creation / recognition failure surfaces as
/// [`OcrBridgeError::PermanentlyUnavailable`] — the same terminal class the
/// daemon and Vision paths use for "this host cannot OCR" — so `--json`
/// reports an `OCR_UNAVAILABLE` code rather than a silent empty result. A
/// successful pass that simply finds no text returns an empty `Vec`.
pub(crate) fn recognize(png: &[u8]) -> Result<Vec<OcrDetection>, OcrBridgeError> {
    // WinRT runtime-class activation (BitmapDecoder/OcrEngine) needs an
    // initialized COM apartment on the *calling* thread. `spawn_blocking` hands
    // us an arbitrary pool thread, so initialize MTA here; ignore the result
    // (`S_FALSE` when already initialized, `RPC_E_CHANGED_MODE` when the thread
    // is already STA — the agile OCR objects work either way). We never
    // uninitialize: the pooled thread keeps its apartment for reuse.
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    recognize_inner(png)
        .map_err(|e| OcrBridgeError::PermanentlyUnavailable(format!("Windows.Media.Ocr: {e}")))
}

/// The WinRT call sequence, with every step funnelled through
/// `windows::core::Error` so [`recognize`] owns the single error-class mapping
/// and the apartment init.
fn recognize_inner(png: &[u8]) -> windows::core::Result<Vec<OcrDetection>> {
    // PNG bytes → an in-memory WinRT stream the BitmapDecoder can read.
    let stream = InMemoryRandomAccessStream::new()?;
    let writer = DataWriter::CreateDataWriter(&stream)?;
    writer.WriteBytes(png)?;
    // Commit the buffered bytes to the stream, then detach so dropping the
    // writer does not close the stream out from under the decoder, and rewind.
    writer.StoreAsync()?.join()?;
    let _ = writer.DetachStream()?;
    stream.Seek(0)?;

    // Decode → SoftwareBitmap in Bgra8/premultiplied, the pixel format
    // Windows.Media.Ocr accepts. Converting up front avoids a format-mismatch
    // throw inside RecognizeAsync for PNGs decoded to other layouts.
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let bitmap = decoder
        .GetSoftwareBitmapConvertedAsync(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Premultiplied)?
        .join()?;

    // Engine from the user-profile languages, falling back to en-US OCR (the
    // language present by default on the Windows images this targets). Both
    // `TryCreate*` calls return Err on a null engine (no OCR pack for the
    // language), so the fallback chain is just `?`-with-recovery.
    let engine = match WinOcrEngine::TryCreateFromUserProfileLanguages() {
        Ok(engine) => engine,
        Err(_) => {
            let language = Language::CreateLanguage(&HSTRING::from(FALLBACK_LANGUAGE))?;
            WinOcrEngine::TryCreateFromLanguage(&language)?
        }
    };

    let result = engine.RecognizeAsync(&bitmap)?.join()?;

    // Per-word detections (see module docs). Index explicitly over the
    // `IVectorView`s so a COM error in `GetAt`/`Size` propagates rather than
    // silently truncating the iteration.
    let lines = result.Lines()?;
    let mut detections = Vec::new();
    for li in 0..lines.Size()? {
        let words = lines.GetAt(li)?.Words()?;
        for wi in 0..words.Size()? {
            let word = words.GetAt(wi)?;
            let text = word.Text()?.to_string();
            if text.is_empty() {
                continue;
            }
            let rect = word.BoundingRect()?;
            detections.push(OcrDetection::new(
                text,
                f64::from(rect.X),
                f64::from(rect.Y),
                f64::from(rect.Width),
                f64::from(rect.Height),
                SYNTHETIC_CONFIDENCE,
            ));
        }
    }

    Ok(detections)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blank 16×16 white RGBA PNG (shared with `vision.rs`). Exercises the
    /// decode → engine → empty-results path without asserting on recognizer
    /// output (that belongs to the live Windows harness OCR band). Only runs
    /// when this crate's tests are executed *on a real Windows host*; the
    /// cross-build from a Mac compiles and links it but never runs it.
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
