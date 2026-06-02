# 6. `screen record` uses a per-platform video-encoder seam (native macOS, ffmpeg elsewhere)

Date: 2026-06-02

## Status

Accepted

## Context

`screen record` is the last `unimplemented!()` stub in the Rust CLI's canonical
surface (`surface.rs` declares the `screen-record` schema; `main.rs` dispatches
both `screen record` and the `record` alias to `unimplemented("screen record")`).

Two facts about the Swift parity baseline reshape how it should be ported:

1. **The Swift recorder encodes with native AVFoundation, not ffmpeg.**
   `cli/Sources/TestAnywareDriver/Capture/StreamingCapture.swift` uses
   `AVAssetWriter` + `AVAssetWriterInputPixelBufferAdaptor` with codec
   `.h264`/`.hevc` — i.e. VideoToolbox, hardware-accelerated, zero external
   dependency on macOS. The root grove brief's instruction to use "embedded
   libav (`ffmpeg-next`), not a subprocess" was therefore **never parity**; it
   was an implicit cross-platform-encoder choice.

2. **The Swift recorder ran inside the shared-VNC `_server`.**
   `RecordCommand.swift` is a thin client: `ServerClient.ensure` →
   `client.recordStart(...)` → sleep for the duration → `client.recordStop()`.
   The frame capture + encode loop lived in the long-lived `_server` process.
   ADR-0004 **deletes** that server. So the Rust recorder cannot be a thin
   client — it must own the RFB stream itself.

Meanwhile the grove has already established a **per-platform native-facility**
direction: ADR-0002 (per-platform `OcrEngine` seam) and ADR-0003 (native macOS
Apple Vision via objc2) reversed the earlier "EasyOCR everywhere" decision in
favour of the best native facility per platform behind a `#[cfg]` seam. The
encoder is the same shape of problem.

## Decision

**`screen record` encodes through a per-platform `VideoEncoder` seam, mirroring
`OcrEngine`:**

- **macOS:** native **AVFoundation / VideoToolbox via objc2** (the same FFI
  strategy ADR-0003 chose for Apple Vision). This is true parity with the Swift
  recorder, is hardware-accelerated, and keeps **ffmpeg out of the primary,
  locally-built arm64-macOS bundle**.
- **Linux / Windows:** **`ffmpeg-next`** (embedded libav) behind the same seam.

**The macOS encoder is built first** — it is the only target verifiable in this
environment (the live-VM gate is macOS/tart). The `ffmpeg-next` encoder is
Tier-2 work that couples to the Windows-host cross-platform pass.

**RFB lifecycle:** the recorder becomes the **second long-lived RFB consumer**
after the embedded viewer (ADR-0005). It reuses that ADR's pattern — a dedicated
RFB connection driven as a continuous `FramebufferUpdate` loop — but is
**bounded by `--duration` and non-interactive** (no input forwarding), so it is
simpler than the viewer: connect → pull updates → feed each frame to the encoder
at the target `--fps` → stop at duration. This reconciles with ADR-0004: every
*other* command stays short-lived per-invocation; record is a bounded long-lived
consumer, not a persistent multiplexer.

This **revises the root grove brief's "embedded libav (`ffmpeg-next`), not a
subprocess" line** — `ffmpeg-next` is retained for Linux/Windows only, and macOS
uses the native path.

## Considered Options

- **`ffmpeg-next` everywhere (the root brief as written).** One code path,
  single encoder to test. Rejected: adds the ffmpeg native dependency +
  cross-compile burden to the primary macOS build, discards the
  hardware-accelerated native path the Swift tool had, and contradicts the
  per-platform-native direction (ADR-0002/0003).
- **Subprocess ffmpeg.** Explicitly excluded by the root brief, and reintroduces
  external-process lifecycle management the port is shedding.
- **macOS-native only, leave Linux/Windows record stubbed.** Closes the macOS
  `unimplemented!()` but leaves a non-macOS gap; folded instead into the Tier-2
  `ffmpeg-next` leaf so the seam is honoured.

## Consequences

- A `VideoEncoder` trait/seam joins `OcrEngine` as a per-platform facility; the
  objc2 FFI surface grows from Vision (`VNRecognizeTextRequest`) to also cover
  AVFoundation (`AVAssetWriter`, pixel-buffer adaptor, `CMSampleBuffer`
  timing) — more FFI than Vision, decided in the `100-screen-record-encoder-macos`
  leaf.
- `ffmpeg-next` enters the dependency tree **for Linux/Windows builds only**,
  compounding the cross-compile cost ADR-0005 flagged for `wgpu`. Both are
  inputs to the distribution leaves (cross-compile feasibility spike, then the
  Tier-2 cross-compile distribution).
- The embedded viewer is no longer "the only long-lived RFB consumer" — that
  glossary line in `CONTEXT.md` (`Embedded viewer`) is updated when the recorder
  lands, not before.
- `screen-record`'s contract envelope (`schema_id: "screen-record"`, `mutating`,
  `data_producing`) is already declared; the implementation must satisfy it
  (`--json`, `--dry-run`, stable error codes, help template).
