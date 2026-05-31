# 030-show-menu

**Kind:** work

## Goal

Implement `agent show-menu` / `agent snapshot --open-menu <path>` in the Rust
CLI, closing the last clean command-parity gap. The VNC primitives it needs
**already exist** — this is a port of the Swift `MenuBarLocator` orchestration,
not a new subsystem.

## Context

The stub's premise is stale. The current error
(`commands/agent.rs::run_snapshot`, ~line 66) says `--open-menu requires VNC
support, which is not yet ported` / "Until the RFB client crate lands" — but the
RFB client landed and already powers `input *` and `screen *`.

The Swift implementation is a 5-step orchestration:
1. snapshot the windows (agent HTTP) — `client.snapshot()` exists in Rust.
2. locate the first menu-bar element matching a label (case-insensitive DFS).
3. derive the element's center click-point from `position` + `size`.
4. **VNC click** at that point — `testanyware-rfb::input::click()` exists.
5. re-snapshot to capture the now-open menu.

Swift splits the *pure* parts into `MenuBarLocator` (side-effect-free,
unit-testable) and keeps the VNC click + re-snapshot in the caller.

- Swift reference: `cli/Sources/TestAnywareDriver/Agent/MenuBarLocator.swift`
  (`findElement(byLabel:)` DFS, `centerPoint(of:)`, `parsePath(_:)` comma-split)
  and `cli/Sources/testanyware/AgentCommand.swift` (`openMenuBarPath` caller).
- Existing Rust primitives to wire: `testanyware-rfb::input::click(...)` (see
  `crates/testanyware-rfb/src/input.rs`); the agent snapshot client and
  `SnapshotResponse`/`WindowInfo`/`ElementInfo` types (`testanyware-protocol`).
- `--open-menu` is a multi-segment path (`"File, Open Recent"`): parse with the
  `parsePath` semantics, then click each segment in order, re-snapshotting
  between segments so the next segment's element exists in the tree.
- Coordinate space: menu-bar element frames are screen-absolute; clicks are
  VNC-absolute. Check whether a window offset applies (see
  `commands/window.rs::resolve_window_offset`, already used by `input` clicks).

## Done when

- `agent snapshot --open-menu <path>` and the `agent show-menu` alias open the
  menu via VNC click(s) and emit the resulting snapshot, matching Swift behavior
  and the `agent-action` schema.
- The pure locate/center/parse logic is ported with unit tests (no live VM
  needed), mirroring Swift's testable split.
- The `ACTION_UNSUPPORTED` / "not yet ported" stub is gone; the stale "until the
  RFB client crate lands" comment is removed.
- `cli-contract.rs` clause for `agent show-menu` passes; `cargo build` clean.
- End-to-end against a live VM is **deferred to leaf 050** (the live-VM gate) —
  this leaf proves it with unit tests + a clean build, not a running guest.

## Notes

This is the only remaining Swift-parity command in the wave; ZRLE/Tight (040),
the live-VM gate (050), and the egui viewer (060) are all *beyond* Swift parity.
