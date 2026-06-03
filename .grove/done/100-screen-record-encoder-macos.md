# 100-screen-record-encoder-macos

**Kind:** work

## Goal

Implement `screen record` (and the `record` alias) on **macOS** via a
per-platform `VideoEncoder` seam — closing the **last `unimplemented!()` on the
macOS surface**. Governed by **ADR-0006**.

## Context

- Encoder: **native AVFoundation / VideoToolbox via objc2** (the FFI strategy of
  ADR-0003's Apple Vision work). True parity — Swift used `AVAssetWriter`
  (`cli/Sources/TestAnywareDriver/Capture/StreamingCapture.swift`) — and keeps
  `ffmpeg` out of the primary macOS bundle. Define the `VideoEncoder` trait/seam
  (mirror `OcrEngine`) so the Tier-2 `ffmpeg-next` encoder plugs in cleanly.
- RFB: the recorder becomes the **second long-lived RFB consumer** after the
  viewer (ADR-0005). Reuse that pattern — dedicated RFB connection, continuous
  `FramebufferUpdate` loop — but **bounded by `--duration`, non-interactive** (no
  input forwarding). Reconciles ADR-0004 (every other command stays short-lived;
  the old Swift recorder lived in the now-deleted `_server`).
- Surface: `screen-record` schema is already declared in `surface.rs`
  (`mutating`, `data_producing`). Swift options to match: `--output`
  (default `recording.mp4`), `--fps` (30), `--duration` (0 = max 300s),
  `--region x,y,w,h`.
- FFI surface grows beyond Vision: `AVAssetWriter`,
  `AVAssetWriterInputPixelBufferAdaptor`, `CMSampleBuffer` timing. Decide
  objc2-direct vs a thin Swift shim here.

## Done when

- `screen record` + `record` alias produce a valid `.mp4` from the live guest on
  macOS, honouring `--fps`/`--duration`/`--region`.
- Satisfies the **CLI design contract** (`docs/architecture/cli-design-contract.md`):
  `--json` envelope, `--dry-run`, stable error codes, help-text template.
- `cli-contract.rs` no longer sees a `screen record` stub; no `unimplemented!()`
  remains on the macOS surface.
- `VideoEncoder` seam is defined and documented so leaf `100`'s Tier-2 sibling
  (ffmpeg-next, linux/win) drops in without reshaping.
- Verified by an actual recording (live-VM gate or manual macOS check).
- `CONTEXT.md` `Embedded viewer` entry updated: it is no longer "the only
  long-lived RFB consumer".

## Notes

- Don't modify the VM image for this (memory [[minimal-images]]).
- May become a node (`leaf-decompose`) if the FFI + RFB-loop split into two
  sessions; decide once into it.
