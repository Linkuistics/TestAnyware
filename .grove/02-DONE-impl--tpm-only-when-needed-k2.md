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

## Decisions (running log)

**The decision is `tpm::decide_tpm(Platform) -> Result<TpmDecision, VmError>`.**
`TpmDecision` is `Attach` or `Omit(OmitReason)`, and the only reason today is
`GuestDoesNotNeedOne`. It returns a `Result` although nothing refuses yet: the
design leaf's *refuse* answer becomes its `Err`, and every caller already takes
`decide_tpm(..)?` and asks `.attaches()`. Its *omit, because the host cannot
supply one* answer is one more `OmitReason`. Neither addition reshapes a caller.
`Platform::needs_tpm` is gone; the decision's table is the only place that
knows Windows needs a TPM.

**Both builders take the socket as `Option<PathBuf>`.** `Some` emits the triple
and `None` omits it, so a TPM device with no swtpm behind it cannot be written.
The triple lives once in `tpm::device_args`. The caller derives the `Option`
from the decision: it launches swtpm and gets `Some(socket)` exactly when the
decision says attach. The builders' tests pin that the two vectors differ by
exactly the six-argument triple, in place.

**Golden creation decides too.** `create_golden_windows` calls
`decide_tpm(Platform::Windows)`, which attaches today. The swtpm preflight, the
swtpm launch and the move of the TPM state into the golden are gated on it, so
D4's remedy needs no second TPM rule.

**`QemuRunner::start` changed narrowly (D8).** Three guards (preflight, state
copy, swtpm launch) and the `Option` socket. Teardown is unchanged, as the
brief's Notes say.

**Taskfile:** `cli:build`, `cli:test` and `cli:clippy`, copied from the
`gh-issue-3` line (header comment reworded). Left out because this leaf runs
none of them: `cli:fmt`, `scripts:check`, the `vm:*` live checks, and the
release tasks.

**Docs sweep.** I listed every TPM/swtpm mention in every tracked non-Rust
file and classified each. Every one names Windows or "TPM-backed guests",
which is still true, except the QEMU start flow in
`docs/architecture/vm-lifecycle.md`. That flow spawned swtpm for every QEMU
guest, and the doc now says only a guest that needs a TPM gets one. Its
hand-synced copy under `website/docs/` got the same edit. The historical plan
under `docs/superpowers/plans/` is left as a record.

**Verification.** `task cli:test` exits 0: 550 tests passed, 0 failed, and the
live-VM tests stay ignored. `task cli:clippy` reports nothing in the files
touched. A live start is not possible here (fact 4, and the TPM abort on this
Mac is the design leaf's), so the argument-vector tests are the evidence.
