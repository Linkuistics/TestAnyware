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
