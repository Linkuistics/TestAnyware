# macos-process-fixtures-review-k188

**Reviews:** macos-process-fixtures-k185

## Goal
Adversarially review the owned process fixtures of the macOS agent tests and
their survival control, before leaf k186 runs `agent:macos:test` on the
shared host.

## Context
- Read the producer commit for `macos-process-fixtures-k185`, its decision
  log, this grove's root brief and the k185 section of
  `docs/experiments/exec-reliability-macos-completion.md`.
- The fixture is
  `agents/macos/Tests/TestAnywareAgentTests/OwnedProcessFixture.swift`. The
  converted tests are in `ProcessRunnerTests.swift` in the same directory.
- The control is
  `experiments/exec-reliability/evidence/macos-process-fixtures-k185/fixtures.py`.
  The Taskfile tasks are `diagnostic:macos-process-fixtures` and
  `diagnostic:macos-process-fixtures-tools`. Raw runs are in the same
  directory.
- The producer did not change `ProcessRunner.swift`. The suite fails in some
  runs with the k3 completed-then-timed-out shape, in both arms.

## Done when
- Challenge whether the fixture can signal a process that the test did not
  start. Cover PID reuse and a stale or partial PID file. Also cover a child
  that exits before tracking and a teardown after a thrown test.
- Challenge whether the control can pass without the property. Confirm that
  the broad pattern selects the control. Confirm that the fault arm runs the
  baseline test file. Confirm that the refusal guard stops the fault arm when
  an unrelated process matches.
- Challenge the claim that the intermittent suite failures do not come from
  the fixtures, using the raw runs of both arms.
- Record each finding with its evidence. Cut an integration leaf only for
  findings worth acting on.

## Notes
