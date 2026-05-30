# 010-agent-action-parity

**Kind:** work

## Goal

Retire the eight pure-HTTP `agent` action stubs in the Rust CLI so they reach
parity with the Swift CLI: `set-value`, `focus`, `wait`, and
`window-{focus,resize,move,close,minimize}`. These talk to the in-VM agent over
HTTP only — **no VNC dependency** (that's `agent show-menu`, deferred).

## Context

- Rust agent-client crate currently exposes only `health, windows, snapshot,
  inspect, press, exec, upload, download` —
  `cli-rs/crates/testanyware-agent-client/src/` (see the `pub async fn` list).
  This task adds the missing action methods.
- CLI dispatch stubs to retire: `cli-rs/crates/testanyware-cli/src/main.rs`
  (`AgentAction::{Wait,SetValue,Focus,WindowFocus,WindowResize,WindowMove,
  WindowClose,WindowMinimize}` → `unimplemented(...)`).
- Swift reference for transport + response shapes + endpoints:
  `cli/Sources/testanyware/AgentCommand.swift` and the `AgentTCPClient`
  methods it calls (`setValue`, `windowFocus`, `windowResize`, `windowMove`,
  etc.) under `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift`.
- Command specs (mutating/data-producing/schema): `surface.rs` —
  the action commands use schema `agent-action`, the window-* use
  `agent-window-action`.

## Done when

- All eight commands are implemented and dispatch real agent-client calls
  (no `unimplemented()` remaining for them).
- Each satisfies the CLI design contract for its class: `--json` output against
  its declared schema, `--dry-run` for the mutating ones, stable error codes,
  help-text template with examples.
- `cli-contract.rs` passes for these commands; unit tests cover the new
  agent-client methods (wiremock per the existing crate test pattern).
- `cargo test --workspace` green; `cargo clippy` clean.

## Notes

`agent show-menu` is intentionally **excluded** — it opens menu-bar items via
VNC click and belongs with the VNC-input work. Keep `wait` semantics aligned
with the Swift `agent wait` (poll the agent until an element/condition holds);
confirm the exact predicate + timeout flags against the Swift implementation.
