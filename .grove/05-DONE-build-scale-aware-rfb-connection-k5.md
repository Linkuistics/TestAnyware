# build-scale-aware-rfb-connection-k5

**Kind:** work

## Goal

Build the **reusable, mechanism-independent core**: make `testanyware-rfb`'s
`RfbConnection` scale-aware, presenting a **logical** framebuffer surface to every
consumer over a larger **physical** wire framebuffer — exact 2:1 downsample on
reads, ×2 pointer scale on writes, scale auto-detected. Unit-testable on **any**
host (no Retina host needed); k6 wires it into `vm start`.

## Context

Read first: **ADR-0016** (D2 — the chokepoint design; start here), `CONTEXT.md`
`[[HiDPI logical framebuffer]]` + `[[Framebuffer-pixel contract]]`.

The chokepoint (from this grove's k3 exploration):
- `RfbConnection` owns the only `Framebuffer` (`cli-rs/crates/testanyware-rfb/`):
  ServerInit size read at `connection.rs:133`; `framebuffer_size()` at
  `connection.rs:188`; `Framebuffer{width,height,RGBA}` at `framebuffer.rs:13`;
  pixel decode in `read_framebuffer_update()` (`connection.rs:307`); pointer writes
  in `pointer_event(mask,x,y)` (`connection.rs:257`) — `x,y` are u16 framebuffer px.
- High-level input helpers (`input.rs` `click/drag/move/scroll`) and CLI input
  (`commands/input.rs` `run_click`, `clamp_coords`) all pass framebuffer px to
  `pointer_event` with **no scaling** today.
- Every framebuffer consumer reads through the connection: `screen size/capture`
  (`commands/screen.rs`, `testanyware-vm/src/capture.rs`), `screen record`
  (`commands/record.rs`), the viewer (`commands/viewer.rs` `fb_pixel` at ~797),
  vision via `screen find-text`.

Design (D2):
- The connection negotiates the **physical** size on the wire (e.g. 3840×2160) but
  is given a **logical target** (e.g. 1920×1080). `scale = physical_w / logical_w`
  (integer; assert physical is an exact multiple — only integer 2× is in scope).
  `scale == 1` ⇒ everything is a **no-op** (graceful on a 1× / non-HiDPI session).
- Reads: `framebuffer_size()` returns **logical**; the consumed pixel buffer is the
  **downsampled** logical image (exact box-average of each `scale×scale` block —
  RGBA, the `image` crate is already a dep via `encode_png`). Decide whether to
  downsample eagerly per update or lazily on read — watch the ~33 MB/frame cost
  (ADR-0016 consequence; the viewer/record loops are hot).
- Writes: `pointer_event`/`click`/etc. accept **logical** coords and multiply
  ×`scale` → physical before the wire. Keep a **physical-bypass** path so `screen
  capture --physical` (k6) can still read the raw frame.

## Done when

- `RfbConnection` carries a logical target + auto-detected scale; `framebuffer_size`
  and consumed pixels are logical; pointer writes scale logical→physical; `scale==1`
  is a verified no-op (existing 1× behaviour byte-identical).
- A raw/physical accessor exists for k6's `--physical` capture/record.
- **Unit tests** (no VM): feed a synthetic 3840×2160 frame → assert 1920×1080
  logical read + correct box-average pixels; assert a logical click maps to the
  expected physical coords; assert scale=1 is a pass-through.
- The downsample method (exact 2:1 box-average) and its per-frame cost posture are
  documented at the call site (driving.md: cite framework choices in code).

## Notes

This is the high-reuse value of the grove and is fully testable now. It must not
change 1× behaviour (the ADR-0013/0014 default path) — scale=1 is the regression
guard. Mechanism-independent: it does not know or care whether the 2× came from the
`pt` path (k4/k6) or a future deterministic fork.
