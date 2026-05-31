# 040-rfb-encodings — brief

## Goal

Add **ZRLE** and **Tight** rectangle decoders to the RFB client crate
(`testanyware-rfb`), which today decodes only Raw + CopyRect. These are
*beyond* Swift parity (the Swift CLI also did only Raw/CopyRect) — they cut
bandwidth and improve `screen capture` / `screen record` fidelity and the
egui viewer's responsiveness over real VNC servers that prefer compressed
encodings.

## Done when

- `testanyware-rfb` negotiates and decodes ZRLE and Tight rectangles, selected
  via `SetEncodings`, producing identical framebuffer output to the Raw path on
  the same content.
- Both decoders have unit tests over captured/synthetic rectangle byte streams
  (no live VM required at this level — live verification rolls up into leaf 050).
- `cargo build` + crate tests green; existing Raw/CopyRect capture unaffected.

## Decomposition

Split by encoding because they share almost no decode logic and Tight is
substantially the harder of the two:

- `010-zrle` — zlib-backed tiled encoding. One persistent zlib stream across the
  whole connection; 64×64 tiles with per-tile sub-encodings (raw / solid /
  palette / RLE). Self-contained and well-specified.
- `020-tight` — the complex one: a zlib-stream-per-channel model with
  basic/fill/gradient/JPEG compression types, palette + copy filters, and a
  JPEG sub-path. May itself need decomposing into a node when reached.

ZRLE first: it establishes the per-connection zlib-stream plumbing and the
tile/sub-encoding dispatch shape that Tight elaborates on.

## Pointers

- RFB encoding type codes: `crates/testanyware-rfb/src/proto.rs::encoding` (only
  `RAW`/`COPY_RECT` + two pseudo-encodings today — add `ZRLE = 16`, `TIGHT = 7`).
- Raw/CopyRect decode + framebuffer write path: the existing rectangle loop in
  `testanyware-rfb` (`connection.rs`, `lib.rs`) and `PixelFormat::rgba32_le()`
  (proto.rs) for the target pixel layout.
- RFB spec §7.7.x (ZRLE, Tight) — the authoritative wire formats.
- A zlib/inflate dependency (e.g. `flate2`) will be needed; check the workspace
  `Cargo.toml` before adding.

## Notes

Independent of the show-menu (030) and server-retire (020) leaves — orderable
anywhere after them, sequenced here so the live-VM gate (050) can exercise the
new encodings against a real server. Keep the pixel-output contract (RGBA into
the framebuffer) byte-identical to the Raw path so capture/OCR are unaffected.
