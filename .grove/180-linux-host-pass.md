# 180-linux-host-pass

**Kind:** work (may decompose if facility wiring splits)

## Goal

Make the `testanyware` host CLI **build and run correctly on Linux** — the
lighter of the two cross-platform host passes. Cfg-wiring, per-platform paths,
and Linux facility selection (OCR engine, video encoder, Vulkan/wgpu). This is
the source side of the Linux story; `190` proves it actually runs in a guest.

## Context

Per the `080` spike and the root brief, Linux is **lighter than Windows**:
`process.rs`/`qemu_profile.rs` already carry the **Unix** path, and the hard
native deps already cross-link (`ring`, `wgpu` — `dlopen`-ed Vulkan, no link-time
sysroot). The work is mostly *selecting* the right facility per `#[cfg]` and
fixing Linux-specific paths, not new architecture (the seams exist).

Facilities to wire / confirm at their seams:

- **OCR engine** (`cli-rs/crates/testanyware-ocr-client`, the `OcrEngine` seam +
  ADR-0002): Linux uses **EasyOCR via the OCR daemon** (`OcrChildBridge` — the
  retained Python-child scaffold, CONTEXT.md *OCR daemon*). Confirm the Linux
  arm is selected and the daemon spawns/locates correctly on a Linux host.
- **Video encoder** (`testanyware-video`, ADR-0006): the `ffmpeg-next` arm from
  `170` — confirm it's selected on Linux.
- **Embedded viewer / wgpu** (ADR-0005): Vulkan is `dlopen`-ed at runtime
  (`080`); confirm the viewer opens on a Linux host with a GPU/llvmpipe.
- **Paths** (`paths.rs` and friends): XDG-style dirs on Linux vs macOS
  `~/Library`; per-platform config/cache/data locations.
- **`doctor`** (`doctor.rs`, already `#[cfg(unix)]`): confirm its Linux preflight
  checks are meaningful (the right tools/paths for a Linux host).

The Windows-only un-gated seam the spike found (`monitor.rs` `UnixStream`) is
**not** a Linux concern (Linux *is* the Unix path) — leave it for the deferred
Windows pass.

## Done when

- `testanyware` builds clean for `aarch64-unknown-linux-gnu` and
  `x86_64-unknown-linux-gnu` (leaning on `160`'s matrix proof).
- The Linux facility arms are correctly selected: OCR→EasyOCR/daemon,
  encoder→ffmpeg-next, viewer→wgpu/Vulkan; paths resolve to Linux locations.
- Any Linux-specific `#[cfg]` gaps or path bugs found are fixed (or, if they need
  a live host to surface, handed to `190` with a note).
- No behaviour regression on macOS (the `#[cfg]` arms stay additive; macOS build
  + `cli-contract.rs` stay green).
- **Runtime correctness on Linux is verified by `190`** (this leaf is the source
  pass; the harness is the proof). Record the green here once `190` confirms.

## Notes

- Honour [[rust-port-conditional-facilities]]: per-platform native facility via
  `#[cfg]`, not lowest-common-denominator.
- Don't modify VM images ([[minimal-images]]).
- If the EasyOCR/daemon wiring or the Vulkan viewer turns into its own session,
  `leaf-decompose` this into a small node — but start as a single leaf (lazy).
