# macos-exit-observation-trace-k198

**Reviews:** macos-exit-observation-instrumented-k195

## Goal

Adversarially review the k195 trace seam, its controls, the traced guest run
and the report claims, before a later leaf relies on them.

## Context

- Read the producer commits for `macos-exit-observation-instrumented-k195`,
  its decision log, this grove's root brief and the k195 section of
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- The seam is `agents/macos/Sources/TestAnywareAgent/ExitTrace.swift`, with
  the `EXIT_TRACE` blocks in `ProcessRunner.swift`. The host controls are
  `agents/macos/Tests/TestAnywareAgentTests/ExitTraceControlTests.swift`
  and `experiments/exec-reliability/evidence/macos-exit-observation-k195/controls.py`.
- The guest tooling is `experiments/exec-reliability/exit_k195.py`,
  `exit_k195_check.py`, `exit_k195_run.sh`, `test_exit_k195.py` and the
  Taskfile entries `diagnostic:macos-exit-observation{,-controls,-check,-tools}`.
- Evidence: `controls-01`, `run-01` and `run-01-outer` under
  `experiments/exec-reliability/evidence/macos-exit-observation-k195`.

## Done when

- The default build is checked for any behaviour change from the seam.
- The seam is read for an ordering that it can record wrongly or miss. The
  read covers PID reuse, a missed kqueue registration and the clock source.
- The checker and its controls are read for failures that a passing control
  would hide. The read covers the route judgement and the call matching.
- Each claim of the k195 section is checked against the retained evidence.
- Findings are recorded in this leaf's commit. An integration leaf is cut
  only for findings worth acting on.

## Notes

Inspection only. Do not repair the tooling in this leaf. Do not run a VM.
