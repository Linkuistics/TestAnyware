# macos-process-fixtures-k185

<!-- tc: off because the creating session wrote this charter for a later session -->

## Goal
Replace the broad name-based cleanup in the macOS agent process tests with
owned, positively identified fixtures, so that `agent:macos:test` is safe to
run on a shared host.

## Context
- Split from `macos-completion-k3` (its Done-when item on ownership-scoped
  process-test cleanup). The k3 report is
  `docs/experiments/exec-reliability-macos-completion.md`.
- `agents/macos/Tests/TestAnywareAgentTests/ProcessRunnerTests.swift` runs
  `pkill -f 'sleep 30'` at lines 76 and 88–89. That command kills any
  matching process on the host, including processes this test did not start.
- `macos-timeout-inspection-k4` depends on this cleanup.
- Do not change `ProcessRunner.swift` or any production wait or cancel
  semantics.

## Done when
- Each test that starts a long-lived child identifies it positively, for
  example by a unique marker argument or a recorded PID and start time. Each
  test signals only that child.
- A control starts an unrelated `sleep 30` outside the test and shows that it
  is still alive after the suite. A fault control restores the broad cleanup
  and shows that the control then fails.
- `agent:macos:test` passes with the owned fixtures, and the existing
  expectations (trimmed output, timeout shape) still hold.
- Record the raw runs under `experiments/exec-reliability/evidence/` and add
  the result to the k3 report.

## Notes
