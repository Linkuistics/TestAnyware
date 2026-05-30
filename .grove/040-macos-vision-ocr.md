# 040-macos-vision-ocr

**Kind:** work

## Goal

Implement the native macOS **Apple Vision** OCR engine and plug it into the
`OcrEngine` seam, so macOS does OCR in-process (no Python daemon) by
default. This completes ADR-0002's per-platform direction: macOS → Vision,
Linux/Windows → EasyOCR daemon. After this lands, the macOS-default-Vision
behaviour documented in `docs/reference/env-vars.md` becomes literally true.

## Context

- The seam already exists: `cli-rs/crates/testanyware-ocr-client/src/engine.rs`,
  `OcrEngine::detect()` has a `#[cfg(target_os = "macos")]` block with a
  `TODO(040-macos-vision-ocr)` marker. Today both arms return the daemon.
  This task adds an `OcrEngine::Vision(..)` variant and returns it on macOS
  unless `TESTANYWARE_OCR_FALLBACK=1` (then daemon).
- `engine_name()` must return `"vision"` for the Vision variant (already in
  the `screen-find-text` schema enum and the `OcrResponse.engine` field).
- Swift reference: `cli/Sources/TestAnywareDriver/OCR/VisionOCREngine.swift`
  (~50 lines: `VNRecognizeTextRequest`, `.accurate`, confidence ≥ 0.5,
  bbox flipped from Vision's bottom-left origin to top-left pixel space).
- **Open decision (decide in this leaf, raise an ADR if hard-to-reverse):**
  the FFI strategy — pure-Rust `objc2` + `objc2-vision` bindings, vs.
  compiling `VisionOCREngine.swift` into a static lib via `build.rs` and
  linking over a C ABI. This was deliberately deferred from 030 (ADR-0002).
- Coordinate space must match `screen capture` / the EasyOCR path: framebuffer
  pixels, top-left origin. Vision returns normalized bottom-left-origin boxes;
  port the Swift flip (`y = (1 - originY - height) * imageHeight`).

## Done when

- macOS builds an `OcrEngine::Vision` engine that OCRs PNG bytes in-process
  via Apple Vision under `#[cfg(target_os = "macos")]`; non-macOS is
  unaffected and still compiles.
- `TESTANYWARE_OCR_FALLBACK=1` on macOS selects the daemon; unset selects
  Vision. Honour `TESTANYWARE_OCR_PYTHON` for the daemon path.
- `screen find-text --json` on macOS reports `engine: "vision"` with
  detections in framebuffer-pixel, top-left-origin coordinates.
- The `env-vars.md` macOS-Vision-default note is now accurate; ADR-0002's
  "not yet literally true" caveat is resolved (amend or supersede it).
- `cargo test --workspace` green; clippy clean. Live-VM verification of an
  actual Vision OCR pass on a macOS guest belongs to the live-VM gate.

## Notes

Builds on the `OcrEngine` abstraction and the wired `screen find-text`
surface from `030-screen-find-text`. The FFI choice sets the precedent for
other macOS-native facilities in the port (e.g. AVAssetWriter for `screen
record`), so weigh it accordingly.
