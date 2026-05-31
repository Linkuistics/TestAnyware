# 010-tart-runner

**Kind:** work

## Goal

Port the **tart** VM backend so the Rust CLI's `vm start/stop/list/delete` can
clone+start the kept-built **tart** goldens (`testanyware-golden-linux-24.04`,
`testanyware-golden-macos-tahoe`) on a macOS host — mirroring the existing QEMU
backend. Today `vm start --platform macos` returns `BackendUnsupported`
(`testanyware-vm/src/lifecycle.rs:126`) and `--platform linux` only knows the
QEMU path. After this leaf, the live-VM gate (`030`) can reach a cheap golden.

## Context

The QEMU backend is the model to mirror:
- `testanyware-vm/src/lifecycle.rs` — `VmLifecycle::{start,stop,delete,list}`.
  The macOS arm of `start` (line 126) and the `VmTool::Tart` arm of `stop`
  (line 206) currently bail with `BackendUnsupported`; these become the tart
  branches.
- `testanyware-vm/src/qemu.rs` — `QemuRunner` (start/stop/clone, golden + clone
  scanning, VNC/agent port discovery). The new `tart.rs` mirrors its shape.
- `testanyware-vm/src/meta.rs` — `VmMeta` / `VmTool` **already has `Tart`**
  (used today only for *reading* Swift-started tart metas); this leaf makes it
  writable from `start`.
- `testanyware-vm/src/spec.rs` — the `VmSpec` (vnc/agent/platform) the gate and
  `screen`/`input`/`agent` commands resolve against (`resolve.rs`).

tart is a **subprocess CLI**, like QEMU — no FFI. Likely commands:
`tart clone <golden> <id>`, `tart run <id>` (with VNC), `tart ip <id>`,
`tart stop <id>`, `tart list`, `tart delete <id>`. The Swift CLI's tart paths
(`cli/Sources/.../VMLifecycle.swift` tart branch) are the porting reference.

### Open questions to resolve at this leaf's bootstrap (grill if needed)

- **VNC endpoint discovery.** tart's guest VNC comes from Virtualization.framework
  (`tart run --vnc` / `--vnc-experimental` prints a `vnc://…` URL with a
  generated password). How does the runner capture host/port/password into the
  `VmSpec`? (QEMU picks an ephemeral port itself; tart hands one back.)
- **Agent endpoint.** The in-VM agent is reached over the guest IP, not
  localhost-forwarded as with QEMU. Use `tart ip` — but **`tart-ip-lies`**:
  `tart ip` returns a cached/stale IP; gate liveness on the `tart list` **state**
  column, and treat the IP as ready only once the guest is `running`.
- **Lifecycle of the `tart run` process.** Unlike QEMU (a backgrounded PID we
  track in meta), `tart run` is a long-lived foreground process. Decide how the
  runner backgrounds it and what PID goes in `VmMeta.pid` for `stop`/liveness.
- **Scope of platforms.** macOS tart golden is the priority (it unblocks the OCR
  + menu-bar checks); the Linux tart golden is a bonus target. Windows stays
  QEMU. Keep the tart module `#[cfg(target_os = "macos")]`-gated so non-macOS
  builds never reference it (consistent with ADR-0003's per-target gating).

## Done when

- `vm start --platform macos` (and `--platform linux` when a tart golden is the
  base) clones+starts the tart golden, waits for the agent, and writes
  `VmSpec`/`VmMeta` sidecars with a usable VNC endpoint and agent endpoint.
- `vm stop <id>` tears down a tart clone and removes its sidecars; `vm list`
  enriches tart clones; `vm delete` handles tart goldens.
- The cli-contract.rs `vm` slice still passes; new unit tests cover the tart
  meta/spec round-trip. A manual `vm start --platform macos` against the real
  golden is verified (clone+start is cheap — `vm-costs`).
- The tart module is macOS-gated; `cargo check --target
  x86_64-unknown-linux-gnu` stays clean (no tart symbols in a Linux build).

## Notes

This supersedes the standalone "tart runner" item in the root brief checklist —
that item is now owned by this leaf. If the VNC-discovery or process-lifecycle
questions turn out to carry a real, precedent-setting trade-off (as the OCR FFI
choice did in ADR-0003), raise a short ADR; otherwise keep decisions inline.
