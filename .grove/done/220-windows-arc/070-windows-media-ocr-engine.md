# 070-windows-media-ocr-engine

**Kind:** work

## Goal

Build the Windows arm of the `OcrEngine` seam decided in `060` (**ADR-0011**): a
`#[cfg(windows)]` `OcrEngine::WindowsMediaOcr` variant backed by native
`Windows.Media.Ocr` via the pure-Rust `windows` crate, and run the **Windows
harness OCR band green** on aarch64-windows — closing the band `220/040` deferred
and bringing Windows to OCR parity (3/3) with Linux and macOS.

## Context

ADR-0011 settled the decision tree: engine = native `Windows.Media.Ocr`; FFI =
the `windows` crate (WinRT); `detect()` returns `WindowsMediaOcr` unconditionally
on Windows (no `TESTANYWARE_OCR_FALLBACK`). This leaf is the localized addition at
the seam — not a rewrite. The reference implementation for the *shape* of an
in-process native engine is `testanyware-ocr-client/src/vision.rs` (the macOS
Vision arm, ADR-0003); mirror its structure (a `#[cfg]` module with a synchronous
`recognize` wrapped by `spawn_blocking` in `engine.rs`).

## Done when

- **Fail-fast link check first** (the `160`-style gate): prove the `windows` crate
  **links** for `aarch64-pc-windows-gnullvm` via `cargo-zigbuild` from this Mac
  before building the engine on top. The crate bundles import libraries for all
  Windows arches so it should link, but verify the cross-from-Mac link early — a
  link failure here changes the whole approach.
- `OcrEngine::WindowsMediaOcr` variant added (`engine.rs`), reported token
  `"windows_media_ocr"` in `engine_name()`; `detect()` returns it unconditionally
  on Windows; `recognize()`/`shutdown()` arms wired (shutdown is a no-op — no
  subprocess, like Vision). All Windows-only code behind `#[cfg(windows)]`; the
  `windows` dep under `[target.'cfg(windows)'.dependencies]` so non-Windows builds
  never compile it (mirror ADR-0003's per-target gating; confirm with a Linux
  `cargo check`).
- `recognize()` implemented (a `src/windows_ocr.rs` module, analogue of
  `vision.rs`): decode the PNG → WinRT `SoftwareBitmap` (via
  `Graphics_Imaging` `BitmapDecoder`) → `Windows.Media.Ocr.OcrEngine`
  (`TryCreateFromUserProfileLanguages` or an explicit `Globalization.Language`) →
  `RecognizeAsync` → map `OcrLine`/`OcrWord` `BoundingRect`s to `OcrDetection`.
  **Coordinate space:** WinRT rects are already image-pixel, **top-left origin** —
  **no Y-flip** (unlike Vision). **Confidence:** `Windows.Media.Ocr` exposes no
  per-word confidence — synthesize a fixed value (e.g. `1.0`) and record the
  choice in a doc comment + ADR-0011 consequences if it surprises. Decide and
  document **per-line vs per-word** detection granularity to match what
  `find-text` consumers expect (cross-check `find.rs` and the EasyOCR/Vision
  granularity).
- **Windows harness OCR band GREEN on aarch64-windows.** Enable the deferred OCR
  band in `tests/windows-host-harness.rs`: `screen find-text <menu-bar-text>
  --json` against the forwarded macOS golden reports `engine ==
  "windows_media_ocr"` and finds the text with a plausible bbox (mirror the
  Linux/macOS gate assertions). No EasyOCR venv is provisioned — the engine is
  in-process WinRT, so the band needs only the cross binary already provisioned by
  `040`'s `ProvisionChannel`. Retire/reconcile the experimental
  `TESTANYWARE_WINDOWS_TRY_OCR=1` knob now that a real engine exists.
- **x86_64-windows: build/link-verified only** (no native x86_64 Windows guest;
  ADR-0009 no-silent-caps — log the gap where a reader sees it).
- Acceptance gate: **CLI design contract** for `screen find-text` behaviour.

## Notes

- ADR: `docs/adr/0011-windows-ocr-via-windows-media-ocr.md` (this leaf's mandate);
  FFI precedent `docs/adr/0003-macos-native-ffi-via-objc2.md`; seam
  `docs/adr/0002-per-platform-ocr-engine.md`.
- Seam code: `cli-rs/crates/testanyware-ocr-client/src/{engine.rs,vision.rs,
  detection.rs,find.rs}`.
- Harness: `cli-rs/crates/testanyware-cli/tests/windows-host-harness.rs`
  (`040` machinery; the OCR band is the deferred third band).
- Cross-build recipe (from `040`/`050`): `cargo-zigbuild` +
  `BINDGEN_EXTRA_CLANG_ARGS=--target=<arch>-pc-windows-gnu`; Windows targets use
  the `-gnu`/`-gnullvm` variants (msvc can't cross from a Mac).
- [[minimal-images]]: provision the binary into a throwaway clone at run time;
  bake nothing test-specific into the Windows golden.
