# start-reports-tpm-k9

## Goal

Make `vm start` say whether the guest's TPM was attached or omitted, and why,
in both its human and `--json` outputs, so the root brief's first done line
("its output says whether the TPM was omitted") holds and a consumer can
check it without reading QEMU's arguments.

## Context

- `tpm-host-check-k7` (before this leaf) gave the decision its full set of
  answers: attach (with or without PPI off), omit because the guest does not
  need one, omit because the host cannot supply one. Refusal is an error and
  is already reported as one. This leaf reports the answers that exist.
- `VmLifecycle::start` builds the decision into `QemuStartOptions.tpm` and
  returns `VmStartResult`; the tart arm (`start_tart`) returns the same type
  and has no TPM decision. `run_vm_start` in `commands/vm.rs` prints the text
  (just the id on stdout) and the JSON envelope, and warns on stderr.
- The contract (`docs/architecture/cli-design-contract.md`) fixes the rules
  that bind this: stdout in text mode stays the id alone, JSON mode puts only
  the envelope on stdout, and every JSON output validates against its schema
  in `docs/reference/cli-schemas/` (`vm-start.json`; `cli-contract.rs` checks
  it). The glossary term is *TPM decision* in `CONTEXT.md`.

## Done when

- The decision's answer reaches the start's result and the two outputs. In
  `--json`, the envelope carries it in a machine-readable form that tells
  attached from omitted and, for omitted, the reason (the guest does not need
  one, or the host cannot supply one). In text mode, a line on stderr says the
  same in words, since stdout stays the id. An omit because the host cannot
  supply one is the case that most needs saying, but attached must be
  reported too: the root brief's final check reads it.
- `vm start --dry-run` reports the same decision in its plan, in text and
  JSON, so the refusal before launch and the decision can be seen without
  starting anything.
- The rendering from decision to JSON value and to text line is a pure
  function with unit tests over every answer. The vm-start schema describes
  the new field, and `cli-contract.rs` passes with the real output.
- The field is optional where it does not apply: a tart-backed start (macOS
  or Linux guests) has no TPM decision, so decide whether it omits the field
  or reports omitted-not-needed, and say why in the Decisions log. The
  recommendation is to omit it: tart has no such decision to report.
- The contract doc records the start's report and the refusal's code
  (`SWTPM_MISSING`, raised before launch); `docs/reference/error-codes.md`
  and its `website/docs/` copy agree. Grep the claim for any other place that
  describes the start's output (`docs/user/`, `docs/architecture/`, the
  `website/docs/` copies, the CLI's help text).
- `task cli:test` passes.
- **Live, on this Mac, on a clone:** `vm start --platform windows --json`
  shows the TPM as attached, and `vm start --platform windows --dry-run
  --json` shows the same. With `swtpm` taken off `PATH` (a subshell with a
  trimmed `PATH` that still finds `qemu-system-aarch64`), `vm start
  --platform windows --dry-run` refuses with `SWTPM_MISSING`, exit 1, and
  names `brew install swtpm`. Stop and delete any clone afterwards.

## Notes

- The exact field name and shape are yours to settle; keep it additive, so the
  schema's `required` list does not grow and existing consumers do not break.
  The contract's section 3.2 permits adding a field in any release, so no
  version bump is needed.
- With the current golden the guest still stops at the lock screen
  (expired `admin` password, stage 2). The start therefore warns that the
  agent never answered, exits 0 and reports the TPM attached. That is
  correct for this leaf. The Healthy agent comes in stage 2.
- Run experiments on clones only. Never modify `testanyware-golden-windows-11`.

## Decisions (running log)

**The JSON shape.** An optional `tpm` object on the `vm start` envelope:
`{"attached": true, "ppi_off": bool}` or `{"attached": false, "reason":
"guest_does_not_need_one" | "host_cannot_supply_one"}`. A boolean tells
attached from omitted without string-matching, `reason` is present only when
omitted, and `ppi_off` only when attached. `vm-start.json` gains `tpm` as a
property; `required` is unchanged, so it is additive under contract §3.2.

**Tart-backed starts omit the field** (the recommendation). `VmStartResult.tpm`
and `StartPlan.tpm` are `Option<TpmDecision>`, `None` on the tart arm: tart
has no decision to report, and "omitted: not needed" would claim one was made.
A QEMU-backed Linux start does make the decision (omit, guest does not need
one), so it reports it; the stderr line is one short line.

**Rendering is two pure functions in `commands/vm.rs`** (`tpm_json`,
`tpm_text`), tested over every answer, plus a test that reads `vm-start.json`
and checks the rendered keys and reason slugs are declared in it. The vm crate
returns the `TpmDecision` untouched; wording and JSON live in the command layer.

**`--dry-run` returns a `StartPlan { backend, tpm }`** in place of the bare
backend name, which no caller used. Text mode prints the decision as a second
`dry-run:` line; JSON carries the same `tpm` object.

**A trimmed `PATH` cannot hide swtpm, so the brief's refusal recipe does not
work as written.** `qemu_profile::which` appends `/opt/homebrew/bin` and
`/usr/local/bin` on macOS (deliberately, for scrubbed environments), and swtpm
is `/opt/homebrew/bin/swtpm`. What works without touching the host's install:
`sandbox-exec` with a profile that denies metadata and data reads of that one
path, for the CLI process only. The control is the same profile making `ls` of
the path fail. It is in the Taskfile as `vm:start-tpm-report-check`. (A first
version of its control used a leading `!`, which never aborts under `set -e`;
caught by running the recipe with the profile pointed at another path, and
fixed with an explicit `if`.)

## Result

**What changed.** `VmStartResult` carries `tpm: Option<TpmDecision>` (`None` on
the tart arm) and `dry_run_validate_start` returns `StartPlan { backend, tpm }`.
`commands/vm.rs` renders the decision with `tpm_json` and `tpm_text`: a `tpm`
object in the `--json` envelope, a stderr line in text mode (stdout stays the
id), and the same in the `--dry-run` plan in both modes. `vm-start.json`
describes `tpm` as optional. Edited: the contract (a paragraph under §3.1), the
`vm start` entry in `cli-commands.md` and its website copy, the start flow in
`vm-lifecycle.md` and its copy, `LLM_INSTRUCTIONS.md`, and the `vm start` help
text. The `SWTPM_MISSING` rows (contract table, `error-codes.md`, website copy)
were already right from k7. ADR-0017 needed no edit.

**Verification.** `task cli:test`: 565 passed, 0 failed (562 before; the three
new ones cover every answer of the renderers and check the output against
`vm-start.json`). `task cli:clippy`: nothing in the files touched.
`cli-contract.rs` has no test that runs a real `vm start`, so "passes with the
real output" is carried by the schema-conformance unit test and the live run.

**Live, on this Mac.** `vm start --platform windows --id k9-live --json`: QEMU
argv held `tpm-tis-device,tpmdev=tpm0,ppi=off`; the envelope carried
`"tpm": {"attached": true, "ppi_off": true}`, `"agent": null`, and stderr had
the expected agent warning; exit 0 (stage 2's expired password). The CLI binary's
digest was the same before and after the run. `--dry-run` (text and `--json`)
reported the same decision. With swtpm hidden from the CLI process (see the
Decisions entry: a trimmed `PATH` cannot do it), `--dry-run` refused with
`SWTPM_MISSING`, exit 1, naming `brew install swtpm`, in text and `--json`. The
clone was stopped; no QEMU or swtpm remains and the golden is untouched.
`task vm:start-tpm-report-check` runs the dry-run checks and the refusal.

**Not exercised live.** The text-mode stderr line on a real (non-dry-run)
start; it is the same `tpm_text` the dry-run text path printed.

**Node close.** `windows-start-on-hvf-k5`: its `Done when` holds across `k7` and
`k9` (the table, the parser, both builders, all three consumers, the report, the
live start at the lock screen). Nothing needed promoting upward.
