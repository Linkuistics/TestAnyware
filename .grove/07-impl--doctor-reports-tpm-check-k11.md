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
