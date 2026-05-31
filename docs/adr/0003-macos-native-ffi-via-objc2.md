# 3. macOS-native facilities via pure-Rust objc2, not a Swift shim

Date: 2026-05-31

## Status

Accepted

## Context

ADR-0002 adopted per-platform OCR engine selection and deferred one
question to the `040-macos-vision-ocr` leaf: *how* the host should call
Apple Vision from Rust. The same question governs every other
macOS-native facility the port will need (AVAssetWriter for `screen
record`, and whatever comes after), so the choice is precedent-setting and
hard to reverse once a second facility copies the first.

Two strategies were on the table:

1. **Pure-Rust `objc2`** — the maintained, auto-generated Objective-C
   framework bindings (`objc2-vision`, `objc2-core-graphics`,
   `objc2-core-foundation`, `objc2-foundation`). Vision is called directly
   from Rust through typed bindings.

2. **Swift shim via `build.rs`** — keep the existing
   `VisionOCREngine.swift`, compile it to a static library in `build.rs`,
   and link it over a hand-written C ABI (`@_cdecl` exports, manual struct
   marshalling).

Both were verified against the actually-resolved crates: every objc2 crate
the Vision path needs (`objc2 0.6`, `objc2-vision 0.3`, `objc2-core-graphics
0.3`, `objc2-core-foundation 0.3`, `objc2-foundation 0.3`, `block2 0.6`)
was already present and resolved cleanly.

## Decision

Use **pure-Rust `objc2` bindings** for all macOS-native facilities,
beginning with the Vision OCR engine (`testanyware-ocr-client/src/vision.rs`).

The objc2 dependencies are declared per-crate under
`[target.'cfg(target_os = "macos")'.dependencies]`, the macOS-only code
lives behind `#[cfg(target_os = "macos")]` modules, and the platform
variant/match arms are likewise cfg-gated — so non-macOS targets never
download or compile the bindings. `cargo check --target
x86_64-unknown-linux-gnu` confirms no objc2 crate enters a Linux build.

The Swift source (`cli/Sources/TestAnywareDriver/OCR/VisionOCREngine.swift`)
remains the porting *reference* — the Rust is a faithful 1:1 of its call
sequence and its coordinate flip — but is not compiled or linked.

## Consequences

- **No Swift toolchain at build time.** The Rust build stays cargo-only on
  every host, which matters for the local-release-from-`scripts/` model (no
  CI) and for the multi-target release matrix in `Cargo.toml`.
- **No C ABI boundary to hand-write or keep in sync.** Memory ownership is
  expressed through objc2's `Retained`/`CFRetained` smart pointers rather
  than manual marshalling, and the bindings are typed against Apple's
  headers.
- **objc2 becomes a load-bearing dependency** for the macOS side of the
  port. It is the de-facto standard for Apple FFI from Rust and is actively
  maintained; the per-target gating contains the blast radius to macOS.
- **Verbosity moves into Rust.** The objc2 call sequence is longer than the
  Swift it replaces (explicit `alloc`/`init`, `unsafe` on property reads
  whose signatures touch other frameworks, normalized→pixel coordinate
  flip done by hand). This is a one-time per-facility cost and is the model
  the next facility will copy.
- **This decision retires the last reason to keep any Swift alive in the
  OCR path**, consistent with the grove goal of deleting `cli/`.
