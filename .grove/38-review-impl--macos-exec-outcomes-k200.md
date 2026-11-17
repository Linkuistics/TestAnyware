# macos-exec-outcomes-k200

**Reviews:** macos-exec-outcomes-k12

## Goal

Adversarially review the k12 exec outcome matrix, its seams, its checker and the
report claims, before `lifecycle-evidence-k8` relies on them.

## Context

- Read the producer commit for `macos-exec-outcomes-k12`, its decision log and
  `docs/experiments/exec-reliability-macos-exec-outcomes.md`.
- The tooling is `experiments/exec-reliability/outcomes_k12.py`,
  `outcomes_k12_seam.py`, `outcomes_k12_check.py`, `test_outcomes_k12.py` and
  `evidence/macos-exec-outcomes-k12/probe/main.swift`.
- The Taskfile entries are
  `diagnostic:macos-exec-outcomes{,-check,-old-path,-tools}`.
- The evidence is `run-01` under
  `experiments/exec-reliability/evidence/macos-exec-outcomes-k12`.

## Done when

- The reference seam is read for any way that it can report an outcome
  wrongly. The read covers the wait status, the group TERM and KILL and the
  temporary-file capture.
- The loopback agent copy is checked against the shipped agent. The check covers
  the byte-identity claim and any behaviour that the new `main.swift` changes.
- The checker is read for a defect class that it can miss or assign wrongly. The
  read covers the `REQUIRED` alternatives and the CLI text mode, which has no
  timeout flag.
- Each claim in the report is traced to a byte or a time in `run-01`.
- Each finding has a severity, a file-and-line anchor and a concrete failure.

## Notes

Do not rerun the matrix in place of `run-01`. A new run goes into a new
directory. The root boundaries apply: no clone, no default socket, no installed
app and no `pkill`.
