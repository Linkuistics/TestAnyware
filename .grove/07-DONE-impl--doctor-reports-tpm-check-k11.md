# doctor-reports-tpm-check-k11

## Goal

`testanyware doctor` reports the host TPM check that `vm start` and
`vm create-golden` already make for Windows guests: whether this host's QEMU
can take a TPM, and whether the PPI must be off. A user whose Windows start
is failing, or who runs a QEMU that cannot supply a TPM, finds that out from
`doctor` instead of from an aborted launch. The human approved this at
`golden-swap-in-k10` (k3 recommended it: it is the same ~20 ms probe and
would have pointed straight at the issue #6 fault).

## Context

- The check lives in `cli-rs/crates/testanyware-vm/src/tpm.rs` (the pure
  decision and the host observation) and is consumed by `qemu.rs` and
  `golden_windows.rs`. `doctor` is `cli-rs/crates/testanyware-cli/src/commands/doctor.rs`.
- Read ADR-0017 and `docs/research/windows-tpm-on-hvf.md` for the rule being
  reported (front-end probe, PPI off under HVF).
- Retired `windows-tpm-on-hvf-k3`: its Notes raised this as a separable idea.
- Reuse the one observation and the one decision; do not re-implement the
  probe inside `doctor` (test seam 1 in the root brief).

## Done when

- `testanyware doctor` (text and `--json`) shows a TPM line for Windows
  guests: attach, attach with PPI off, omit (with reason), or refuse (with
  reason), taken from the same decision `vm start` uses.
- A unit test walks the doctor line for each decision outcome with no QEMU or
  swtpm.
- `docs/architecture/cli-design-contract.md` and the `doctor` entry in the
  user docs (and their `website/docs/` copies) describe the new line;
  `task cli:test` passes.
- A live `testanyware doctor` on this Mac reports the TPM attached with PPI
  off.

## Notes

- Doctor reports; it does not fail the run on an omitted TPM unless the
  decision is "refuse".
- Live evidence is cheap (no VM needed): run `doctor` directly.

## Decisions (running log)

**`doctor` consumes the decision, it does not re-run the probe.**
`DoctorReport::gather` calls `decide_tpm_on_host(Platform::Windows,
&host_profile())`, the entry point `vm start` uses, and a pure
`classify_windows_tpm` turns its `Result` into a `TpmCheck`. `vm.rs`'s
`tpm_json` and `tpm_text` became `pub(crate)` so the JSON fields and the words
are the ones `vm start` emits.

**Status mapping.** `Attach` and `Omit(GuestDoesNotNeedOne)` are `pass`;
`Omit(HostCannotSupplyOne)` is `warn`; `SwtpmMissing` (the decision's refuse
answer) is `fail` and the only outcome that flips `ok` and the exit code. Any
other error (QEMU would not run, or unrecognised probe output) is
`Unobservable`, a `warn`: the host was not observed, which is not a refusal,
and QEMU's absence is already shown under host tools (a tart-only Mac must not
fail `doctor`).

**Doc scope.** The `doctor` user docs exist only in `docs/reference/cli-commands.md`.
The `website/docs/` copy of that file has no `doctor` section or table row at
all (it is stale in other ways too), so there is no entry there to update and
none was added. Updated: the CLI's own `doctor --help` exit-code text, the
contract (§3.1 note and the §10.1 row), `doctor.json` (new required group), the
README comment, ADR-0017 (one sentence).

**Live evidence.** `testanyware doctor` on this Mac prints `✓ Windows guests:
TPM attached (PPI off)`; `--json` gives `{"attached": true, "ppi_off": true,
"status": "pass"}`. `task doctor:tpm-check` also checks the refuse path with
swtpm hidden by a sandbox (control: the sandbox's `ls` fails), where `doctor`
exits 1 with `SWTPM_MISSING`. `task cli:test` passes.
