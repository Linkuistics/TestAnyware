# build-hidpi-optin-and-wiring-k6

**Kind:** work

## Goal

Wire the HiDPI opt-in end-to-end: the **`--display WxH@2x`** surface, its
translation to shipped tart's host-scale `pt` path, suppression of ADR-0014's 1×
guest switch, the scale-aware connection's logical-target hookup (k5), a host-scale
warning, and the `--physical` flag on `screen capture`/`record`. After this, `vm
start --display 1920x1080@2x` on a Retina host renders an app under test at 2× while
vision/clicks operate in logical 1920×1080.

## Context

Read first: **ADR-0016** (D3 — the opt-in surface; start here), the **k4 findings**
(the confirmed `vm start` sequencing — does the guest need a switch to select the
Retina mode?), **ADR-0014** (the 1× switch being suppressed), `CONTEXT.md`
`[[HiDPI logical framebuffer]]`. Depends on **k5** (the scale-aware connection) and
**k4** (the confirmed mechanism + sequencing).

Code sites (from k3 exploration):
- `--display` flows untouched: CLI flag → `VmStartOptions.display` →
  `tart.rs` `resolve_display` (default `1920x1080px`) → `set_display` →
  `tart set --display`. **Add `@2x` parsing/translation here** — strip `@2x`, set
  HiDPI intent + logical target, emit `WxHpt` to tart (current mechanism). tart
  **never sees `@2x`**. Reject `@Nx` for N≠2 (out of scope).
- ADR-0014's 1× switch: `lifecycle.rs:229`-ish calls `display::apply(...)`
  (`testanyware-vm/src/display.rs`) which uploads/execs `set-display-mode.swift`
  selecting `pixelWidth==w && width==w`. **Suppress under `@2x`** — or replace with
  the Retina-mode selector if k4 found the guest needs a guest-side switch to
  *select* the 2× 1920-logical mode.
- The scale-aware connection (k5): set its logical target from the parsed `@2x`
  value wherever connections are opened for a given VM; auto-detect confirms the
  physical came back 2×.
- **Host-scale warn:** if `@2x` was requested but the negotiated framebuffer is
  not 2× the logical (1× host, or HiDPI didn't take), warn clearly (the auto-detect
  scale=1 path stays correct — never silently wrong).
- `--physical` flag: `commands/screen.rs` (`screen capture`) + `commands/record.rs`
  (`screen record`) — emit the raw physical frame via k5's physical-bypass accessor;
  default stays logical.

## Done when

- `vm start --display 1920x1080@2x --platform macos` on a Retina host: app renders
  at 2×; `screen size` reports logical 1920×1080; `screen capture` is 1920×1080 and
  `screen capture --physical` is 3840×2160; a logical `input click` lands correctly
  (×2 on the wire); the viewer shows the logical frame and clicks map correctly.
- ADR-0014's 1× switch is suppressed (or redirected per k4) under `@2x`; the 1×
  default path (no `@2x`) is **unchanged**.
- `@Nx` for N≠2 is rejected with an actionable error; a 1×-host `@2x` warns.
- The `command surface` / help text documents `@2x` (per the CLI design contract /
  `cli-tool-design` standard); integration coverage where feasible.

## Notes

Mechanism-agnostic by construction: when the deferred deterministic fork (ADR-0016
"Deferred") lands, only the `@2x`→mechanism translation changes — the surface, the
connection, and the suppression logic stay. End-to-end Retina-host validation here
depends on k4 confirming the `pt`→2× premise.
