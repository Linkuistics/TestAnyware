# windows-start-on-hvf-k5 — brief

## Goal

Stage 1 of the remedy: ADR-0017. A Windows guest, and Windows golden creation,
start again on an Apple Silicon host whose Homebrew QEMU is 11.1 or newer,
with a working TPM, and the start says whether the TPM was attached or
omitted. This stage fixes the abort in `tpm-tis-device` only. It does not touch
the golden, so with the current golden the guest boots but stops at the lock
screen ("Your password has expired"), which is stage 2's fault
(`windows-golden-password-k6`), not this one's.

## Done when

- The TPM decision (`TpmDecision`, decided by `decide_tpm`) implements
  ADR-0017's table in full, unit-tested with no QEMU or swtpm: not Windows →
  omit (guest does not need one; no probe runs); the QEMU binary lists no
  `tpm-tis-device` → omit (host cannot supply one); swtpm missing → refuse with
  `SWTPM_MISSING` before cloning; otherwise attach, with `ppi=off` under HVF
  when the binary lists a `ppi` property.
- The host check reads `<qemu> -machine none -device tpm-tis-device,help` and
  the parser is unit-tested on the three captured texts (11.1.2, 11.0.1, "not
  found").
- Both argument builders emit `ppi=off` exactly when the decision says so, and
  the TPM triple exactly when it says attach (seam 2).
- `vm start`, `vm start --dry-run` and `vm create-golden --platform windows`
  all consume the one decision. A golden with no `<golden>-tpm` state starts
  on a fresh swtpm state rather than failing the copy.
- `vm start` and `vm start --json` say whether the TPM was attached or
  omitted, and why; the contract, schema and docs record it.
- Live on this Mac: `vm start --platform windows` keeps QEMU up with
  `tpm-tis-device,tpmdev=tpm0,ppi=off` and reaches the lock screen (the
  expired-password prompt with the current golden is expected and is stage 2).

## Decomposition

1. `tpm-host-check` (impl): the host observation, the parser, the full
   decision table, `ppi=off` in both builders, wired into every consumer of
   the decision. This alone restores Windows starts on HVF, so it comes first.
2. `start-reports-tpm` (impl): carries the decision to the start's two outputs
   and records it in the contract. It follows because it reports the answers
   the first leaf adds.

## Pointers

- `docs/adr/0017-qemu-guest-tpm-decided-from-the-qemu-binary.md` (the table
  and why) and `docs/research/windows-tpm-on-hvf.md` (evidence, E1 to E3, the
  reproduction recipes). Read the ADR first.
- Retired `tpm-only-when-needed-k2`: the decision's current shape and the
  `Option<PathBuf>` socket convention in both builders. Its Decisions say why
  the decision already returns a `Result`.
- `task vm:qemu-tpm-check` (`scripts/qemu-tpm-check.sh`): reads the same
  property list; it is the cheapest check that a host's QEMU runs the TPM
  device as the CLI attaches it.
- Glossary: TPM decision, Healthy agent, Golden image (`CONTEXT.md`).
- Seams: root brief seams 1 and 2 (pure decision; both builders).

## Notes

- Live checks on a Windows clone cannot reach a Healthy agent in this stage:
  the current golden's `admin` password has expired (k3, F4). Look at the
  lock screen over VNC (`testanyware screen capture` against the clone's
  endpoint) and at QEMU's argv.
- Run experiments on clones only. Never modify `testanyware-golden-windows-11`.
- This branch lacks issue #3's honest start: an early QEMU exit says only
  "QEMU did not remain running after launch" and teardown deletes the clone's
  `qemu.log`. Run QEMU by hand, or keep the log, to see its reason.
