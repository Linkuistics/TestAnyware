# 030-viewer-reconnect-and-start-sugar

**Kind:** work

## Goal

Lifecycle polish for the viewer: (1) **auto-reconnect** so the window survives
guest reboots / transient drops, and (2) the **`vm start --viewer` sugar** so the
one-gesture "start VM and open a window" flow works again. Both are additive on
top of leaves 010/020.

## Done when

- **Auto-reconnect:** on a `next_message()`/`connect` error, the RFB thread loops
  back through `resolve_vnc` + `RfbConnection::connect` with bounded backoff
  instead of exiting; the UI shows a "reconnecting…" overlay (replacing leaf
  010's terminal "disconnected" overlay) and resumes rendering on success. A
  ceiling / explicit give-up path is defined (don't spin forever silently).
- **`vm start --viewer` sugar:** the no-op warning in `commands/vm.rs`
  (`"--viewer is not yet ported (backlog task 8)"`) is replaced — after the VM is
  up and the `vm-start` envelope is emitted, the viewer is opened inline for that
  VM's resolved endpoint (blocking-until-close, per ADR-0005 / Q2). Interaction
  with `--json` is sane (envelope emitted before the window blocks).
- `cli-contract.rs` passes; `vm start --viewer --dry-run` still reports the plan
  without opening a window.
- Verified on the macOS primary host: bouncing the VM (stop/start) reconnects
  the open viewer; `vm start --viewer` opens a working window.

## Notes

- Reconnect reuses the exact connect path leaf 010 already built — this leaf adds
  the retry loop + overlay state, not new connection code.
- Keep backoff modest (the viewer is a dev tool, not a service); surface the
  give-up as a clear terminal overlay + non-zero exit if appropriate.
