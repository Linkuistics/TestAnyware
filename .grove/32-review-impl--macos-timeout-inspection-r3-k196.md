# macos-timeout-inspection-r3-k196

**Reviews:** macos-timeout-inspection-k193

## Goal

Adversarially review the k193 r3-only run and its claims before the
instrumented leaf k195 builds on them.

## Context

- Read the producer commits for `macos-timeout-inspection-k193`, its decision
  log, this grove's root brief and the k193 section of
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- The tooling is `experiments/exec-reliability/timeout_k193.py`,
  `timeout_k193_check.py`, `timeout_k193_run.sh`, `test_timeout_k193.py`,
  the `check` parameters added to `timeout_k4_check.py` and the Taskfile
  entries `diagnostic:macos-timeout-inspection-r3{,-check,-tools}`.
- Evidence: `experiments/exec-reliability/evidence/k193-r3-macos-01` and
  `k193-r3-macos-01-outer`.

## Done when

- Each claim of the k193 section is checked against the retained evidence.
  The checks cover the plan identity with the original r3 sequence, the
  child absences, the cancel child survival and the clone disposal.
- The new `check` parameters are read for a change to the k4 judgement.
- The r3-only checker and its controls are read for failures that a
  passing control would hide.
- Findings are recorded in this leaf's commit. An integration leaf is cut
  only for findings worth acting on.

## Notes

Inspection only. Do not repair the tooling in this leaf.
