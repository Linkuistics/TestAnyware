# implement-default-resolution-k2

**Kind:** work

## Goal

Make `vm start` apply a default of **1920×1080 px** when `--display` is omitted,
resolved per-backend, and verify the **RFB framebuffer** actually reports
1920×1080 px on each platform. Implements ADR-0013.

## Context

Read ADR-0013 (`docs/adr/0013-default-guest-display-resolution.md`) — it is the
binding decision and explains the tart pt/px asymmetry. Key code sites (from the
plan-k1 recon, verify line numbers — they may drift):

- `cli-rs/crates/testanyware-vm/src/tart.rs` — `set_display` (~:209) runs
  `tart set --display`; called from `TartRunner::start` (~:355) only when
  `opts.display.is_some()`. **macOS default must emit `1920x1080px`** (explicit
  `px`; bare string is *points* → 2× framebuffer under HiDPI).
- `cli-rs/crates/testanyware-vm/src/qemu.rs` — gpu geometry (~:124) builds
  `virtio-gpu-pci,xres=W,yres=H` only when `spec.display.is_some()`. **Default
  must emit `xres=1920,yres=1080`.** Note the parser splits the display string on
  `x`, so do *not* feed it a `px`-suffixed string.
- `cli-rs/crates/testanyware-cli/src/main.rs` — the `--display` flag (`Option
  <String>`, ~:1800). Help text must document the new default.

Resolve the default **per-backend** (each backend owns its encoding) — not as a
single static clap default, which can't be both `1920x1080px` (tart) and
`1920x1080` (qemu). Leave a user-supplied `--display` value untouched.

`screen size` reads back the negotiated framebuffer dimensions — that's the
verification probe. `testanyware-rfb/src/connection.rs:131` reads w/h from
ServerInit.

## Done when

- `vm start` with no `--display` yields a **1920×1080-px framebuffer** on each
  backend, confirmed via `screen size` against a running golden:
  - **Linux** (QEMU): mechanically certain via `xres/yres` — confirm.
  - **Windows** (QEMU): same path — confirm.
  - **macOS** (tart): the real check — does VF honor `1920x1080px` exactly, or
    snap to a nearby mode? Eyeball LoDPI render fidelity vs the synthetic
    training look. If VF won't yield exactly 1920×1080 px, record the finding,
    amend ADR-0013, and (if needed) spawn a follow-up leaf.
- A user-supplied `--display WxH` is still honored unchanged.
- `--display` help text documents the default.
- Test suite green (watch for any framebuffer-size assumptions; check the
  cli-contract / golden tests).

## Notes

- Confirm tart `--display-refit`/`--no-display-refit` doesn't perturb a fixed
  headless VNC session; set `--no-display-refit` explicitly if it does.
- A 1920×1080-px framebuffer pushes ~8 MB/frame uncompressed — watch viewer /
  `screen record` for any perf regression, but it's expected acceptable.
- Per `driving.md` source-discipline: re-read `tart set --help` / the live code
  before relying on the line numbers above; cite the tart pt/px behavior in a
  one-line code comment at the macOS default site.
- Golden VMs are kept built (clone+start is cheap) — verification per platform is
  low-cost.
