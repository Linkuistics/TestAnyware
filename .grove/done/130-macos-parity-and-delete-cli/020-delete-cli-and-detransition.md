# 020-delete-cli-and-detransition

**Kind:** work

## Goal

With macOS parity proven by `010`, perform the Tier-1 terminal retirement:
**delete `cli/`** (the macOS-only Swift CLI), **de-transition `CONTEXT.md`**, and
clean up Swift-referencing docs. Runs only after `010` lands.

## Context

- `cli/` is macOS-only (`Package.swift`: `platforms: [.macOS(.v14)]`), so Tier-1
  completion *is* the parity bar (Decision 070, Q5). Tier-2 Linux/Windows
  additive work then proceeds on the clean tree.
- ~106 files under `cli/` (incl. the `_server`/`Server/` Shared-VNC tree that
  ADR-0004 noted would be removed wholesale here).

## Done when

- **Re-verify parity first**: `cargo test --test cli-contract` green (full surface,
  no remaining `#[ignore]` on the offline sweeps that `010` enabled). Spot-check a
  couple of the commands `010` fixed.
- `cli/` deleted (all Swift files, incl. `_server`/`Server/`). `git rm -r cli/`.
- `CONTEXT.md` de-transitioned:
  - `Host CLI` — drop "Currently in transition…"; Rust is now *the* host CLI.
  - `Swift CLI` — either remove the entry or rewrite as a historical note
    ("retired 2026-… ; lived under `cli/`"). Keep the term only if docs/code
    still reference it.
  - `Rust CLI` — drop "in-progress replacement"; it *is* the host CLI.
  - `Shared-VNC server` — note the Swift `_server` is now gone (ADR-0004 done).
  - `Command surface` — the `_server` example is moot; adjust if needed.
- Swift-referencing docs handled:
  - `docs/components/cli.md` — retire or rewrite for the Rust CLI.
  - `docs/reference/error-codes.md` — update/retire (Rust `surface.rs` ERROR_CODES
    is the live catalogue now).
  - Release scripts under `scripts/` — confirm no Swift references remain
    (double-checks leaf `120`, which ported the release pipeline).
  - Grep the repo for stragglers: `rg -i 'cli/Sources|Package.swift|swift build|\bSwift CLI\b'`.

## Notes

- The grove does **not** finish here — Tier 2 (`140` + its children) remains.
- No ADR for the deletion — recoverable via git; the sequencing is already a
  root-brief note (Decision 070). The node `BRIEF.md` records this too.
- After this leaf retires and the `130` node empties, the retire step promotes
  any surviving brief context upward and `mv`s `130` into `done/`.
