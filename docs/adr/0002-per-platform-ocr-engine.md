# 2. Per-platform OCR engine, reversing "EasyOCR everywhere"

Date: 2026-05-30

## Status

Accepted

## Context

The host CLI's `screen find-text` needs OCR over a captured guest
framebuffer. Two facts collided while porting it to Rust:

1. The Rust `testanyware-ocr-client` crate was written asserting a single
   canonical engine: *"The Apple Vision OCR fallback is intentionally not
   ported … EasyOCR is the canonical engine on every platform."* That was a
   deliberate simplification when the primary host target was Linux.

2. The wider Rust-port direction (`port-swift-cli-to-rust` BRIEF; the
   `conditional-facilities` memory) is the opposite: each platform should
   use its **best native facility** via `#[cfg(target_os = …)]`. For OCR
   that means **Apple Vision in-process on macOS**, EasyOCR daemon
   elsewhere — which is also what the Swift CLI did (`VisionOCREngine` +
   `OCRChildBridge`) and what `docs/reference/env-vars.md` already
   documents (`TESTANYWARE_OCR_FALLBACK=1` forces the daemon on macOS).

This is a hard-to-reverse, surprising trade-off — it reverses a *written*
decision and sets the precedent for how every per-platform native facility
in the Rust port is structured — so it is recorded here rather than buried
in a code comment.

## Decision

Adopt **per-platform OCR engine selection**, reversing "EasyOCR
everywhere":

- macOS → in-process Apple **Vision** by default; the EasyOCR daemon when
  `TESTANYWARE_OCR_FALLBACK=1`.
- Linux / Windows → the EasyOCR Python **daemon** (`OcrChildBridge`).

Selection lives behind an `OcrEngine` abstraction in
`testanyware-ocr-client` (`engine.rs`). `OcrEngine::detect()` carries the
`#[cfg(target_os = "macos")]` seam where the Vision arm plugs in;
`recognize()`/`engine_name()`/`shutdown()` dispatch over the variant. The
reported `engine` token (`"vision"` | `"easyocr_daemon"`) is part of the
`screen-find-text` JSON schema.

**Staged delivery (grove lazy decomposition).** The native macOS Vision
engine is *not* built in this increment. `screen find-text` ships now with
every platform routing through the daemon; the macOS Vision implementation
(an `objc2`/Vision-vs-Swift-shim FFI choice in its own right) lands in a
follow-up leaf, `040-macos-vision-ocr`, as a localized addition at the
`detect()` seam — not a rewrite. Until then, macOS users get daemon-backed
OCR, consistent with `TESTANYWARE_OCR_FALLBACK` semantics.

## Consequences

- The `testanyware-ocr-client` doc comment that asserted the old decision
  is corrected to point at the engine seam and this ADR.
- `OcrChildBridge` remains load-bearing scaffold (not residue) for the
  daemon engine and the wider vision pipeline; it is not deleted.
- A follow-up leaf owns the macOS Vision FFI. Its biggest open choice —
  pure-Rust `objc2`/`objc2-vision` bindings vs. compiling the existing
  `VisionOCREngine.swift` via `build.rs` — was deferred to that leaf.
  **Resolved in ADR-0003: pure-Rust `objc2`.**
- The macOS-default-Vision behaviour documented in `env-vars.md` was not
  literally true when this ADR was written (macOS used the daemon).
  **Resolved by leaf `040-macos-vision-ocr`:** `OcrEngine::detect()` now
  returns the in-process `OcrEngine::Vision` on macOS unless
  `TESTANYWARE_OCR_FALLBACK=1`, so `env-vars.md` is accurate and this
  caveat is closed.
