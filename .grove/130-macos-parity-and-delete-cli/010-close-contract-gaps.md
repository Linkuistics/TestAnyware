# 010-close-contract-gaps

**Kind:** work

## Goal

Bring the 14 commands the parity sweep flagged up to **full CLI design contract**
compliance, then **enable the offline full-surface contract tests** so
`cli-contract.rs` is genuinely green for the whole canonical surface (not just
structurally). This is the parity proof that gates deleting `cli/` in `020`.

See the node `BRIEF.md` for the full sweep table and decisions.

## Work

All edits are in `cli-rs/crates/testanyware-cli/src/main.rs` (help consts +
command structs) and `tests/cli-contract.rs` (enable sweeps), unless noted.

1. **§7 help template** — add an `AFTER_HELP` const + `#[command(after_long_help
   = …)]` for each of: `input key/key-down/key-up/type/click/mouse-down/mouse-up/
   move/scroll/drag` (10), `screen capture`, `screen record`, `screen size`,
   `agent show-menu`. Follow the established const pattern (e.g.
   `FILE_UPLOAD_AFTER_HELP`): one-line about (already present), then OUTPUT,
   EXIT CODES, EXAMPLES (≥2 concrete `testanyware …` invocations), SEE ALSO.
   `screen size` is `ConnectionArgs` — give it its own args struct or attach help
   via the enum variant.

2. **§9.3 `--dry-run`** (DECISION: implement, not exempt) — add a `dry_run: bool`
   field to all 10 `input *` variants and to `agent show-menu`, and a plan-only
   branch in each handler (lines ~1481–1600 for input, ~1701 for show-menu): when
   `dry_run`, resolve+validate the connection and emit the same JSON envelope as
   the real run with `"dry_run": true`, exit 0, **without** sending the
   event/opening the menu. Mirror how `file upload`/`vm stop` already do it.

3. **§3.1 `--json`** — `agent show-menu` is a bare struct missing `--json`
   entirely though `surface.rs` marks it `data_producing`. Add `json: bool` and
   wire the JSON envelope (schema `agent-action`).

4. **Enable the offline full-surface contract tests** in `cli-contract.rs`:
   - `each_subcommand_help_follows_template` (§7): replace the `todo!()` with a
     walk over `CANONICAL_COMMANDS` asserting each `--help` contains OUTPUT /
     EXIT CODES / EXAMPLES / SEE ALSO and ≥2 example invocations. Remove
     `#[ignore]`. (`viewer` is exempt from OUTPUT/EXIT CODES per its
     interactive-command carve-out — special-case it like the existing
     per-command tests do, or assert the reduced set.)
   - `each_mutating_command_supports_dry_run` (§9.3): confirm offline-feasibility
     first — does `--dry-run` short-circuit before the network connect when given
     a `--connect <spec>` (or synthetic state like the vm tests use)? If yes,
     implement the sweep and un-ignore. If a command genuinely needs a live
     target to dry-run, leave it ignored but **re-point the reason** at the
     live-VM gate (see below) rather than "as ports land".
   - For the live-gated ignores (`each_data_command_supports_json` §3.1,
     `errors_carry_stable_code_and_correct_exit` §3.4, `identifiers_round_trip`
     §6.1, `list_commands_default_limit_and_truncate` §9.4): update each
     `#[ignore = "…as ports land"]` reason to state it is a **live-VM gate**
     concern (`tests/live-vm-gate.rs`), since the ports HAVE landed — the stale
     "as ports land" wording is now misleading (grove constraint 1: artifacts not
     stale state). Keep the `todo!()` only if a real offline check is still
     possible; otherwise convert the body to a doc-comment pointer.

5. **Schemas** — if `agent show-menu`'s `--json` envelope shape changes anything,
   confirm `docs/reference/cli-schemas/agent-action.json` still validates
   (`schema_command_emits_json_schema_for_each_command` covers this).

## Done when

- `cargo test --test cli-contract` green with the newly-enabled `#[test]`s
  **un-ignored** (offline surface fully covered); no `todo!()` masquerading as a
  finished port.
- The offline parity sweep (re-run the session-start sweep, or the new
  `each_subcommand_help_follows_template`) shows **zero** missing-section /
  missing-`--dry-run` rows for the 14 commands.
- `cargo build` / `cargo clippy` clean.

## Notes

- Keep the help text terse and in the house style; copy phrasing from the
  nearest compliant sibling (`input` ← `file upload`'s pattern; `screen
  capture/record` ← `screen find-text`/the §7.1 skeleton, which literally
  documents `screen capture`).
- Do **not** touch `cli/` or `CONTEXT.md` here — that is `020`.
- This is a meaty but mechanical session. If `--dry-run` for `input` turns out to
  need non-trivial handler refactoring (shared dry-run plumbing), it's fine to
  split a follow-up leaf — but try to land it in one focused commit first.
