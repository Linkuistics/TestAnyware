# tpm-only-when-needed-k2

## Goal

Give a QEMU guest a TPM only when it needs one (D7), through the single TPM
decision the human agreed as a test seam (D5, seam 1). Windows behaviour does
not change on any host: Windows still gets its TPM.

## Context

- Decisions in the retired `plan-k1`: D7 (all four TPM sites), D5 (seams 1 and
  2), D10 (the Taskfile), D8 (base on `main`); facts 3 and 4.
- The two argument builders are `build_qemu_args` (for `vm start`) and
  `build_install_qemu_args` (for `vm create-golden --platform windows`). Both
  are pure and have argument-vector unit tests.
- `QemuStartOptions.needs_tpm` (fed by `Platform::needs_tpm`) already carries
  the guest's need, but it gates only the swtpm preflight.
- Issue #3's `Taskfile.yml` (in the `gh-issue-3` line; `jj file show -r
  gh-issue-3@ Taskfile.yml`) supplies the task names and recipes to reuse.

## Done when

- One pure TPM decision, from the guest's platform, answers *attach* or *omit,
  because the guest does not need one*. Its shape lets the design leaf add the
  host's observation and the *omit, because the host cannot supply one* and
  *refuse* answers without reshaping any caller.
- Every TPM site consumes that answer. The TPM-state copy, the swtpm launch and
  the swtpm preflight run only when it says attach. Both argument builders
  emit the TPM device (the `-chardev` / `-tpmdev` / `-device tpm-tis-device`
  triple) exactly then.
- Unit tests pin the decision's table, and both builders with and without a
  TPM. The existing argument tests still pass.
- Windows still attaches a TPM on every host.
- A root `Taskfile.yml` carries the tasks this leaf ran (at least `cli:test`),
  under issue #3's task names and recipes, and `task cli:test` passes.
- The docs that state who needs swtpm or a TPM have been swept (grep the claim,
  not a file list) and say what is now true.

## Notes

- A Linux-on-qcow2 guest cannot be started live on this Mac: the only QEMU
  golden here is Windows (fact 4). This leaf is verified by its unit tests.
  Because Windows is unchanged, a live Windows start on this Mac still aborts
  at the TPM. That is the design leaf's to fix, not a regression here.
- Teardown finds swtpm by its TPM state directory, so it is already a no-op
  when no swtpm ran, and removing a golden's `-tpm` directory is already
  idempotent. Neither needs to change.
- Issue #3's line also edits `QemuRunner::start`. Keep this leaf's change there
  narrow, so #3's later rebase stays small (D8).
