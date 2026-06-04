# 170-ffmpeg-video-encoder

**Kind:** work

## Goal

Implement the **non-macOS `VideoEncoder` arm** with `ffmpeg-next` so `screen
record` (and the `record` alias) works on Linux and Windows — filling the
`#[cfg(not(target_os = "macos"))]` branch that currently returns
`VideoEncoderError::Unsupported`. Governed by **ADR-0006**; plugs into the seam
`100` already shaped.

## Context

The seam is ready and does not need reshaping (`100` built it for exactly this):

- `cli-rs/crates/testanyware-video/src/encoder.rs` — the `VideoEncoder` trait
  (`append_frame(&mut self, rgba: &[u8])` then `finish(self: Box<Self>)`),
  `VideoEncoderConfig { width, height, fps, codec, output }`, `VideoCodec`
  (`H264`/`Hevc`), and `new_encoder()` whose `#[cfg(not(macos))]` arm is the
  stub to replace.
- Frames are **RGBA, top-left origin** (`Framebuffer::rgba` byte layout) — the
  ffmpeg encoder must convert RGBA→YUV for the codec.
- macOS reference impl: `testanyware-video/src/avfoundation.rs` (objc2
  `AVAssetWriter`). Mirror its lifecycle (setup → per-frame append at
  `frame_index / fps` PTS → finish/flush) and its error→contract-code mapping
  (`Setup`/`Append`/`Finish` → the codes in `VideoEncoderError::code`).
- Add `ffmpeg-next` as a `#[cfg(not(target_os = "macos"))]` dependency in
  `testanyware-video/Cargo.toml`; gate the module in `lib.rs` like
  `avfoundation` is gated.

**Cross-build coupling — this is the leaf that introduces the `160` ffmpeg
risk.** `ffmpeg-next` links system `libav*` at link time via `pkg-config`. The
encoder must build for all four `140`-matrix triples. Coordinate with `160`: if
`160` ran the ffmpeg half throwaway-style, confirm it still links with the real
integration; if `160` deferred ffmpeg, **this leaf owns the ffmpeg cross-link
proof** (run `cargo-zigbuild` for the four triples after wiring the dep).

## Done when

- `new_encoder()`'s non-macOS arm returns a working `ffmpeg-next` encoder;
  `screen record`/`record` produce a valid `.mp4` honouring `--fps`/`--duration`/
  `--region` on Linux. (Windows runtime verification trails the Windows-host
  pass; Linux runtime verification is the `190` harness's job.)
- Satisfies the **CLI design contract** unchanged (the seam already routes
  errors to `--json` codes); no new surface, just a filled arm.
- **Cross-link confirmed** (or the blocker recorded for the distribution
  re-plan): `ffmpeg-next` + the encoder link via `cargo-zigbuild` for the four
  triples, or the sysroot recipe is documented.
- Unit coverage mirrors the encoder's existing tests (frame-size guard, codec
  mapping); a real recording is verified in `190` (Linux) — note it here once
  green.

## Notes

- Don't modify the VM image ([[minimal-images]]).
- Codec default matches Swift: H.264 portable default, HEVC optional.
- The recorder is the **second long-lived RFB consumer** (ADR-0005/0006),
  bounded by `--duration`, non-interactive — the loop already exists on the
  macOS path; this leaf only swaps the encoder behind the seam.
- ffmpeg can be picky about even width/height for some pixel formats — surface a
  `--region`/odd-dimension guard if it bites.

## Outcome (2026-06-04)

- `FfmpegEncoder` (`testanyware-video/src/ffmpeg.rs`) fills the non-macOS arm:
  libx264/libx265 → `.mp4`, swscale RGBA→YUV420P, PTS `frame_index/fps`, same
  `Setup`/`Append`/`Finish` mapping as the AVFoundation arm. `ffmpeg-next = 8.1`,
  `default-features = false` + `["codec","format","software-scaling"]` (4 libs).
  Even-dimension guard added.
- **Cross-link CONFIRMED on all four triples** via `cargo-zigbuild` + BtbN
  prebuilt ffmpeg dev libs — incl. `aarch64-windows` (160's "weakest link").
  Linux = full `testanyware` bin; windows = `testanyware-video` test bin (full
  windows bin still blocked only at the deferred `monitor.rs` gap, not ffmpeg).
  Recipe + per-triple evidence + runtime-ABI options:
  `docs/research/170-ffmpeg-cross-link.md`.
- Unit coverage: `codec_id` mapping + odd-dim guard (in-module); the encode
  round-trip + frame-size guard live in `tests/ffmpeg_smoke.rs` (runs on Linux —
  link-checked here via `cargo zigbuild --tests`; real recording is **190**'s).
