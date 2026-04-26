---
title: Video Recording
---

# Video Recording

`testanyware record` captures the VNC framebuffer of a running VM to a
video file.

## Usage

```bash
testanyware record -o session.mp4 --fps 30 --duration 10
```

Full synopsis:

```
testanyware record [-o <output>] [--fps <fps>] [--duration <duration>] [--region <region>]
```

- `-o, --output <output>` — Output file path. Default: `recording.mp4`.
- `--fps <fps>` — Frames per second. Default: `30`.
- `--duration <duration>` — Duration in seconds. `0` means "use the
  built-in 300 s cap". Default: `0`.
- `--region <region>` — Crop region as `x,y,width,height`. Omit for
  full frame.

Plus the standard connection flags (`--vm`, `--vnc`, `--agent`,
`--platform`, `--connect`).

## What it does under the hood

`StreamingCapture` in `cli/Sources/TestAnywareDriver/Capture/StreamingCapture.swift`
wraps `AVAssetWriter` with an `AVAssetWriterInputPixelBufferAdaptor`.
For each frame:

1. A VNC framebuffer update is requested.
2. The frame is converted from RFB pixel format to a `CVPixelBuffer`
   via `FramebufferConverter`.
3. The pixel buffer is appended to the writer at the computed
   presentation time (`frame_index / fps`).
4. On stop (or duration expiry), the writer is finalised and the
   file is closed.

## Codec

Output is H.264 by default. HEVC is supported via the `Codec.hevc`
option exposed through the `StreamingCapture.Config` struct, but the
CLI currently pins H.264 for maximum compatibility. Container is
MP4.

Typical file sizes at 1920x1080 @ 30 fps:

- H.264: ~2-4 MB per second of desktop activity.
- HEVC: ~1-2 MB per second of desktop activity.

## Capturing a region

```bash
testanyware record -o bottom-right.mp4 --region 960,540,960,540 --duration 10
```

The region is cropped from each frame before encoding. Coordinates
are screen-absolute; use `testanyware screenshot` first to sanity-check
the region.

## Errors

See `StreamingCaptureError` in
[`docs/reference/error-codes.md`](../reference/error-codes.md).
The common ones:

- `alreadyRecording` — another `record` session against the same spec
  is already running.
- `notRecording` — internal; indicates the writer state machine is
  out of sync.
- `pixelBufferPoolUnavailable` — the writer didn't start successfully;
  usually paired with an earlier error on stderr.
