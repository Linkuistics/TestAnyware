# 030-screen-find-text

**Kind:** work

## Goal

Port `screen find-text` to the Rust CLI: capture the guest framebuffer, run OCR,
and report matches (text + bounding boxes) for the requested query. Retire the
`screen find-text` stub at contract parity.

## Context

- Stub to retire: `cli-rs/crates/testanyware-cli/src/main.rs`
  (`ScreenAction::FindText` → `unimplemented("screen find-text")`). Surface
  entry: `screen find-text` (alias `find-text`), schema `screen-find-text`,
  non-mutating, data-producing.
- Building blocks that already exist: the RFB capture path (`screen capture` is
  wired via `testanyware-rfb`) and the `testanyware-ocr-client` crate. This task
  wires capture → OCR → result envelope.
- Swift reference: `cli/Sources/testanyware/FindTextCommand.swift` and the OCR
  stack under `cli/Sources/TestAnywareDriver/OCR/` (`VisionOCREngine`,
  `OCRDetection`, `OCRChildBridge`, `OCRStatusFile`).
- **Per-platform OCR direction** (load-bearing — reverses the old "EasyOCR
  everywhere" call): macOS should use **Apple Vision** via `#[cfg(target_os =
  "macos")]`; Linux/Windows use EasyOCR/other. Check the current
  `testanyware-ocr-client` module comment — it may still assert the old
  everywhere-EasyOCR decision and needs reconciling. See root BRIEF Pointers.

## Done when

- `screen find-text` captures the framebuffer and returns OCR matches as a
  `--json` envelope against the `screen-find-text` schema (text, confidence,
  bounding box, coordinate space consistent with `screen capture`).
- macOS path uses the native Vision facility under `#[cfg]`; the non-macOS path
  compiles and is wired to the OCR client.
- Coordinate/region semantics match `screen capture` and the Swift behavior
  (window-relative inset compensation if applicable).
- `cli-contract.rs` passes for `screen find-text`; `cargo test --workspace`
  green; clippy clean.

## Notes

This leaf may surface the OCR-architecture reconciliation (Vision vs EasyOCR,
the `OcrChildBridge` daemon role). If that reconciliation turns out to be a
hard-to-reverse, surprising trade-off, raise an ADR rather than burying it in
code comments. The `OcrChildBridge` daemon is **scaffold for the vision
pipeline**, not dead residue — do not delete it as part of this task.
