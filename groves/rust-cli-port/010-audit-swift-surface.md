# 010-audit-swift-surface

**Kind:** planning

## Goal
Catalogue every command in the Swift CLI (`cli/Sources/testanyware/`),
map its Rust equivalent (if any) and the precise behavioral gap, and
grow this tree with the next level of structure.

## Context
- Swift CLI sources: `cli/Sources/testanyware/*Command.swift`, plus
  `TestAnywareCLI.swift` (root command) and the `LLMInstructions*` pair.
- Rust CLI command modules:
  `cli-rs/crates/testanyware-cli/src/commands/` — currently agent,
  file, input, screen, vm, window.
- Canonical command surface:
  `cli-rs/crates/testanyware-cli/src/surface.rs`
  (`CANONICAL_COMMANDS`, `SYNONYM_ALIASES`, `VERB_FIRST_ALIASES`) —
  treat this as the source of truth for which command names exist on
  the Rust side.
- Contract: `docs/architecture/cli-design-contract.md` — every Rust
  equivalent must satisfy it.
- Precedent (use as a *shape* reference, not a *content* one):
  `docs/superpowers/plans/2026-05-22-port-qemu-runner-and-vm-lifecycle-to-rust.md`
  — what a chunk-port plan looks like end to end.

## Done when
- An audit produced — either as a new file
  `groves/rust-cli-port/swift-surface-audit.md` or folded back into
  this BRIEF on retirement — that for every Swift command records:
  | Swift command | Behavior summary | Rust equivalent | Gap | Suggested ownership |
- For every commands cluster the audit identifies, either
  (a) a single root-level leaf has been created
      (`020-port-doctor.md`, `030-port-record.md`, …), or
  (b) a sub-node directory exists with its own `BRIEF.md` and ordered
      child leaves (e.g. `020-ocr/BRIEF.md` + `020-ocr/010-…`).
- `CONTEXT.md` has gained any new project-specific terms surfaced
  during the audit (e.g. `ConnectionSpec`, persistent-server roles).
  Inline updates, not batched.
- An ADR has been raised **only** if a hard-to-reverse, surprising
  architectural choice emerges (e.g. "consolidate the `_server`
  runtime into the `agent` command rather than port it as `_server`").
  Most likely none.
- One focused commit lands the new tree + audit file (if any) +
  `CONTEXT.md` deltas.

## Notes
- Already-deferred VM items belong in this grove and in the audit's
  output:
  - tart backend (`vm start --platform macos`)
  - viewer wiring (`vm start --viewer`)
  - `vm create-golden` subcommand (per memory
    `project_golden_creation_in_cli.md`)
  - Windows-host process control (`CREATE_NEW_PROCESS_GROUP` /
    `GenerateConsoleCtrlEvent`)
- The Swift `_server` (`commandName: "_server"`, `shouldDisplay: false`)
  is in scope. It may itself need a dedicated sub-node because it's a
  persistent runtime with its own lifecycle, not a one-shot command.
- The audit's job is *shape*, not *blueprint*. Don't prescribe
  implementation details for the child tasks — that's their job.
  Recommend a decomposition; let the next planning sessions sharpen it.
- LLM-instructions generation (`LLMInstructions.generated.swift` +
  `LlmInstructionsCommand.swift`) is conceptually part of the
  discoverability surface — the Rust side has
  `discoverability::run_llm_instructions`. The audit confirms parity
  or files a gap.
