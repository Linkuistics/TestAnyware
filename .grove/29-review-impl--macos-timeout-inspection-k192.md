# macos-timeout-inspection-k192

**Reviews:** macos-timeout-inspection-k4

## Goal

Adversarially review the k4 post-timeout inspection diagnostic and its claims
before the remainder leaf k193 builds on them.

## Context

- Read the producer commits for `macos-timeout-inspection-k4`, its decision
  log, this grove's root brief and
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- The tooling is `experiments/exec-reliability/timeout_k4.py`,
  `timeout_k4_check.py`, `timeout_k4_run.sh`, `test_timeout_k4.py` and the
  Taskfile entries `diagnostic:macos-timeout-inspection{,-check,-tools}`.
- Evidence: `experiments/exec-reliability/evidence/k4-timeout-macos-01*`
  (refused before acquisition) and `k4-timeout-macos-02*` (the unchanged clone).

## Done when

- Each claim of the report is checked against the retained evidence. The
  checks cover the stall of `after-09-table`, the timeout overheads, the
  kill of the issue child and the clone disposal.
- The checker revision made after run 02 is judged for post-hoc bias.
- The plan, the checker and the owned cleanup are read for failures that a
  passing control would hide.
- Findings are recorded in this leaf's commit. An integration leaf is cut only
  for findings worth acting on.

## Notes

Inspection only. Do not repair the tooling in this leaf.
