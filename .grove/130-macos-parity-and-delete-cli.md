# 130-macos-parity-and-delete-cli

**Kind:** work

## Goal

Tier-1 terminal step: verify **full macOS parity**, then **delete `cli/`** (the
macOS-only Swift CLI, 100 files) and **de-transition `CONTEXT.md`**.

## Context

- `cli/` is macOS-only (`Package.swift`: `platforms: [.macOS(.v14)]`), so Tier-1
  completion (record macOS encoder, vm create-golden macOS, macOS distribution)
  *is* the parity bar — Decision (070, Q5). Tier-2 Linux/Windows additive work
  then proceeds on the clean tree.
- Gate: `cli-contract.rs` passes for the **full** `surface.rs::CANONICAL_COMMANDS`
  with no `unimplemented!()` remaining on the macOS surface. Run a parity sweep
  against `docs/architecture/cli-design-contract.md`.

## Done when

- `cli-contract.rs` green for the full canonical surface; a parity sweep confirms
  each command meets the contract (error codes, `--json`, `--dry-run`, help,
  schema discovery).
- `cli/` deleted (all ~100 Swift files, incl. the `_server`/`Server/` tree
  ADR-0004 noted would be removed wholesale here).
- `CONTEXT.md` de-transitioned: the `Host CLI`, `Swift CLI`, `Rust CLI` entries
  drop the "in transition" framing (Rust is now *the* host CLI); the
  `Shared-VNC server` entry can note the Swift `_server` is gone.
- Swift-referencing docs handled: `docs/components/cli.md`,
  `docs/reference/error-codes.md` retired/updated; release scripts no longer
  reference Swift (folds into / double-checks leaf `120`).

## Notes

- The grove does **not** finish here — Tier 2 (`140` + its children) remains. This
  leaf just removes the old tree once parity is proven.
- Whether the "delete cli/ mid-grove" sequencing deserves its own ADR: decide
  when here. It's recoverable via git, so likely a root-brief note suffices
  (already recorded), not an ADR.
