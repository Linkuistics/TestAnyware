# 010-zrle

**Kind:** work

## Goal

Add a **ZRLE** rectangle decoder to `testanyware-rfb`, negotiated via
`SetEncodings`, producing framebuffer output identical to the Raw path.

## Context

ZRLE (RFB §7.7.6, encoding type `16`):
- A **single zlib stream persists for the whole RFB connection** — not per
  rectangle. The decoder must keep one inflate context alive across rectangles
  and feed each rectangle's zlib-data length-prefixed blob into it. This is the
  key plumbing this leaf establishes (Tight, leaf 020, reuses the per-connection
  zlib-stream idea but with multiple streams).
- After inflation, the rectangle is tiled into **64×64** tiles, row-major. Each
  tile begins with a sub-encoding byte:
  - `0` raw pixels (CPIXEL format),
  - `1` solid colour (one CPIXEL),
  - `2–16` packed palette,
  - `17` unused, `128` plain RLE, `130–255` palette RLE.
- **CPIXEL** is the compressed pixel format: for our negotiated 32bpp
  true-colour `rgba32_le()`, CPIXEL is 3 bytes (the significant channels), not
  4. Get this width right — it is the classic ZRLE bug source.

## Context pointers

- Encoding code: add `ZRLE: i32 = 16` to `proto.rs::encoding`.
- Existing rectangle loop + framebuffer writer (Raw/CopyRect) in
  `connection.rs` / `lib.rs` — plug the ZRLE arm into the same dispatch and
  write into the same RGBA framebuffer via `PixelFormat::rgba32_le()`.
- Add `flate2` (or equivalent) to the workspace if not already present.

## Done when

- ZRLE rectangles decode correctly: per-connection zlib stream, 64×64 tiling,
  all tile sub-encodings (raw/solid/palette/RLE), correct CPIXEL width.
- Unit tests over synthetic and/or captured ZRLE byte streams assert
  pixel-identical output vs. the equivalent Raw rectangle.
- `SetEncodings` advertises ZRLE ahead of Raw so a real server will use it.
- `cargo test -p testanyware-rfb` green; Raw/CopyRect capture unaffected.

## Notes

Live verification against a real VNC server rolls up into leaf 050. Keep the
zlib-stream context owned by the connection object (it must outlive any single
rectangle decode call).
