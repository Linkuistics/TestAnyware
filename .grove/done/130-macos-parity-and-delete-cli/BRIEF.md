# 130-macos-parity-and-delete-cli — brief

**Kind:** node (decomposed 2026-06-03)

## Goal

Tier-1 terminal step: reach **full macOS parity** against the CLI design
contract, then **delete `cli/`** (the macOS-only Swift CLI, ~100 files) and
**de-transition `CONTEXT.md`**.

## Why this is a node now

`130` started as a single work leaf assumed to be "confirm `cli-contract.rs`
green + delete `cli/`". A **parity sweep** at session start (2026-06-03) found
the gate was only *structurally* green: every per-command port satisfied its own
narrow test slice, but the **cross-cutting full-surface contract tests are still
`#[ignore]`d** ("implemented per-command as ports land") — and nobody re-enabled
them once the last port landed. The sweep against the built binary found real
contract gaps, so parity is **not yet met**. Closing those gaps is a distinct
commit from deleting `cli/`, and `cli/` must only be deleted *after* parity is
proven — hence two ordered leaves.

## Parity-sweep findings (offline, vs. `docs/architecture/cli-design-contract.md`)

| Dimension | Affected commands |
|---|---|
| §7 help template missing (OUTPUT / EXIT CODES / EXAMPLES≥2 / SEE ALSO) | all 10 `input *`, `screen capture`, `screen record`, `screen size`, `agent show-menu` (14) |
| §9.3 `--dry-run` missing (marked `mutating: true` in `surface.rs`) | all 10 `input *`, `agent show-menu` (11) |
| §3.1 `--json` missing (marked `data_producing: true`) | `agent show-menu` (1) |

Already compliant (verified): vm/agent(non-show-menu)/file commands; `screen
record` already carries `--dry-run` (needs help only); `file exec` is fine (its
EXIT CODES section is titled "EXIT CODES (text mode):").

The `#[ignore]`d full-surface tests in `cli-contract.rs` that must be enabled:
- `each_subcommand_help_follows_template` (§7) — **offline**, enable after help fixed.
- `each_mutating_command_supports_dry_run` (§9.3) — offline-feasible via `--connect`
  to a spec that dry-run validates before connecting; confirm during `010`.
- The remaining ignored ones (`each_data_command_supports_json` §3.1,
  `errors_carry_stable_code_and_correct_exit` §3.4, `identifiers_round_trip` §6.1,
  `list_commands_default_limit_and_truncate` §9.4) need a **live VM**; their happy
  paths belong to `tests/live-vm-gate.rs`, not the offline contract gate. Update
  their stale "as ports land" ignore reason to name the live-VM gate instead of
  leaving a `todo!()` that implies unfinished port work.

## Decisions

- **`input *` `--dry-run`: IMPLEMENT** (user, 2026-06-03) — satisfy §9.3 as
  written rather than amend the contract to exempt `input`. A dry-run resolves
  the connection and reports the planned event (key/click/move/…) without
  sending it, keeping `input *` consistent with every other mutating command. No
  ADR — it's "comply with the contract as written", not a surprising/irreversible
  trade-off.
- **delete `cli/` mid-grove**: no ADR — recoverable via git; the root-brief note
  already records the sequencing (Decision 070: after macOS parity).

## Children

- `010-close-contract-gaps` — bring the 14 deficient commands to full contract
  compliance (help + `--dry-run` + `--json`), enable the offline full-surface
  contract tests, re-point the live-gated ignores. One commit; `cli-contract.rs`
  fully green (offline surface).
- `020-delete-cli-and-detransition` — final parity re-verification, delete
  `cli/`, de-transition `CONTEXT.md`, handle Swift-referencing docs. Separate
  commit; runs only once `010` proves parity.

## Notes

- The grove does **not** finish here — Tier 2 (`140` + its children) remains.
- Done-when for the node as a whole = the union of the two children's done-when
  (the original leaf's "Done when", now split across `010`/`020`).
