# 020-tight

**Kind:** work

## Goal

Add a **Tight** rectangle decoder to `testanyware-rfb`, negotiated via
`SetEncodings`, producing framebuffer output identical to the Raw path.

## Context

Tight (RFB §7.7.5, encoding type `7`) is the most complex standard encoding.
Each rectangle starts with a **compression-control byte** selecting:
- **Fill** — a single solid colour for the whole rectangle (no zlib).
- **JPEG** — a length-prefixed JPEG blob (decode via an image/JPEG crate).
- **Basic** — zlib-compressed pixel data with an optional **filter**:
  - copy (no filter), palette, or gradient.

State to manage:
- **Up to four independent zlib streams** per connection, selected by a 2-bit
  stream id in the control byte. Each must persist across rectangles (like
  ZRLE's single stream, leaf 010 — but four of them). A "reset" bit per stream
  flushes it.
- **TPIXEL** compressed pixel format (3 bytes for `rgba32_le()` 24-bit colour),
  analogous to ZRLE's CPIXEL.
- The **compactlen** variable-length encoding for zlib-data byte counts.

Because of the JPEG path, basic-vs-fill-vs-jpeg branching, three filters, and
four zlib streams, **this leaf may warrant decomposing into a node** when
reached (e.g. fill+basic-copy first, then palette/gradient filters, then JPEG).
Decide at bootstrap; don't pre-split.

## Context pointers

- Encoding code: add `TIGHT: i32 = 7` to `proto.rs::encoding`.
- Reuse the per-connection zlib-stream plumbing and tile/framebuffer-write
  patterns established by leaf 010 (ZRLE).
- JPEG decode: pick a maintained crate (`jpeg-decoder` / `image`), gate it
  behind the Tight feature if it bloats non-RFB builds.
- RFB spec §7.7.5 for the exact control-byte layout, filter ids, and
  TPIXEL/compactlen rules.

## Done when

- Tight rectangles decode correctly across fill, basic (copy/palette/gradient
  filters), and JPEG compression types; four-stream state persists across
  rectangles with correct reset handling.
- Unit tests over synthetic and/or captured Tight byte streams assert
  pixel-identical output vs. equivalent Raw (with a tolerance note for JPEG's
  lossy path).
- `SetEncodings` advertises Tight; `cargo test -p testanyware-rfb` green;
  Raw/CopyRect/ZRLE capture unaffected.

## Notes

If decomposed into a node, the node retires when all Tight sub-leaves are done;
live verification rolls up into leaf 050 regardless.
