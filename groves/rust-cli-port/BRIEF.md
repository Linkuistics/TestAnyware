# rust-cli-port — brief

## Goal
Retire the Swift CLI in `cli/` by porting every command (visible and
hidden, including the `_server` internal runtime) to the Rust CLI in
`cli-rs/` at strict parity against the CLI design contract, then deleting
`cli/` and dropping the Swift toolchain from CI.

## Done when
- Every `testanyware <command>` the Swift CLI exposes has a Rust
  equivalent satisfying `docs/architecture/cli-design-contract.md`.
- Every contract test under
  `cli-rs/crates/testanyware-cli/tests/cli-contract.rs` passes — no
  `#[ignore]` skeletons left for retired-Swift-only commands.
- `.github/workflows/ci.yml` builds and tests Rust only; no Swift
  toolchain step.
- A single retirement commit removes `cli/` from the repo.

## Decomposition
The decomposition itself is the first leaf. The audit will grow this
tree — either as further root-level leaves (`020-port-X.md`, …) or as
nested sub-nodes (`020-X/BRIEF.md` + ordered children) where commands
cluster naturally.

- `010-audit-swift-surface.md` (planning) — catalogue every Swift
  command, map its Rust equivalent and gap, propose the next layer.

## Pointers
- ADRs to read here: none yet (no architectural decisions recorded for
  this workstream — created lazily when one earns its place).
- Glossary terms in play: Host CLI, Swift CLI, Rust CLI, Command
  surface, CLI design contract, In-VM agent, Golden image (see
  `CONTEXT.md`).
- Existing design docs:
  - `docs/architecture/cli-design-contract.md` — the contract every
    ported command must satisfy.
  - `docs/superpowers/plans/2026-05-22-port-qemu-runner-and-vm-lifecycle-to-rust.md`
    — the just-completed VM-lifecycle port plan; the model for what a
    chunk-port plan looks like (merged at `0634fa6`).

## Notes
- The VM lifecycle (`vm start/stop/list/delete`) is already ported and
  merged (2026-05-22, commit `0634fa6`). The `testanyware-vm` crate
  exists and is green. The audit should treat the `vm` command surface
  as parity-verified-by-default; its only open items are the deferred
  ones called out in the VM-port plan (tart backend, viewer wiring,
  `vm create-golden` subcommand, Windows-host process control).
- "Strict parity" is at command-level *behavior*, not implementation
  shape. Reuse Rust idioms where they read better; don't slavishly
  mirror Swift patterns.
- Single context for now: `CONTEXT.md` at the repo root. A
  `CONTEXT-MAP.md` is introduced only if a future grove (e.g. for
  `agents/`) defines a conflicting term.
- Standard grove layout per `.claude/skills/grove/SKILL.md`. Bootstrap
  decisions for this grove were made during a Q1–Q4 grilling on
  2026-05-23; no separate planning task captures them — they're encoded
  here in the BRIEF.
