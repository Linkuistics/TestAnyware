# increase-default-screen-size — brief

## Goal

Give guest VMs a known, larger default display resolution so the accessibility/
vision testing workload runs on-distribution. Introduce a TestAnyware-owned
default of **1920×1080 pixels**, applied at `vm start` when `--display` is
omitted; `--display` keeps overriding.

## Done when

- `vm start` with no `--display` produces a **1920×1080-px RFB framebuffer** on
  every backend (verified via `screen size` against a running golden per
  platform), and a user-supplied `--display` is still honored unchanged.
- `--display` help text documents the default.
- Test suite is green.

## Decomposition

- **plan-k1** (planning, DONE) — grilled the design; settled motivation,
  mechanism, value, and the macOS pt/px encoding; wrote ADR-0013.
- **implement-default-resolution-k2** (work) — per-backend default + help text +
  per-platform empirical verification.

## Pointers

- **ADR-0013** `docs/adr/0013-default-guest-display-resolution.md` — the binding
  decision (the px-framebuffer contract, the tart pt/px asymmetry, rejected
  2560×1440).
- Recon map of every place resolution is set today: see `01-DONE-plan-k1.md`
  Context + Decisions log.
- Key sites: `cli-rs/crates/testanyware-vm/src/tart.rs` (`set_display`, ~:209,
  :355), `…/src/qemu.rs` (gpu geometry, ~:124), `…/testanyware-cli/src/main.rs`
  (`--display` flag).

## Notes

The real contract is the **framebuffer pixel count** (what `testanyware-rfb`
negotiates and the vision pipeline consumes), not the string handed to the
hypervisor — tart's `--display` unit is a *hint* (pt for macOS, px for Linux),
so the macOS default must carry an explicit `px`.
