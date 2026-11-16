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

## Decisions (running log)

**Identity.** A tracked child is the pair of its PID and its kernel start time
from `proc_pidinfo(PROC_PIDTBSDINFO)`. The start time stays the same across
`exec`, so `echo $$ > file; exec sleep 30` keeps one identity.

The test shell
writes the PID into a file in a directory that only one fixture uses. The
fixture accepts the PID only when the process started after the fixture was
created and runs `sleep`. I rejected a marker argument, because finding it
again needs a scan by command line, which is the shape this leaf removes.

**Scope.** Only the two tests that ran `pkill -f 'sleep 30'` change.
`testTimeoutReturnsTimedOutFlag` also starts `sleep 30`, but `ProcessRunner`
stops that child itself, and the test signals nothing. `ProcessRunner.swift`
is unchanged (hash `f9401669…6340` before and after every run).

**Cleanup.** Cleanup runs as an XCTest teardown block, so it runs when the
test fails. It sends SIGTERM, waits 2 s, then sends SIGKILL and waits 2 s,
within the brief's bounds.

It signals only while the PID still carries the
recorded start time. A PID can still be reused between that check and
`kill`. Darwin has no process handle that closes this window.
The fixture documents the window.

**Control shape.** The control runs `sleep 300`. The old pattern matches it
as a substring, so the broad cleanup selects the control. The fault arm
restores the test file of `990641750f` in a temporary copy of the package. The
working tree never contains the broad cleanup again. The fault arm refuses to
run when another process of this user matches `sleep 30`, because the restored
cleanup would stop it.

**Suite result.** Some runs of `agent:macos:test` fail. The failing tests are
`testHandlesLargeOutputWithoutPipeDeadlock` and `testCapturesStdoutAndExitCode`.
Each one reports `timedOut` with exit -1 after a command that completes in
0.07 s alone.

The owned arm failed in 7 of 20 runs and the broad arm in 4
of 20 (Fisher p = 0.48). `testCapturesStdoutAndExitCode` fails before any
fixture code runs. These runs do not show that the fixtures cause it. The
receipt shape is the k3 completed-then-timed-out shape.

This leaf cannot repair it, because the
charter excludes `ProcessRunner.swift`. So the third criterion holds for the
fixture tests and the existing expectations, but the suite is not reliably
green. The k3 report records this item as remaining k3 work.

**Session deviation.** During a log check, I ran `pkill -f "^du -sh"` to stop
a stalled `du` of my own. That is a name-based kill, which the root brief
forbids. The pattern matches only a command line that starts with `du -sh`.
The `du` had already finished, and I saw no effect. I stopped the stalled
shell through its task handle instead.

**Review.** The fixture sends signals on a shared host, and the suite result is
new evidence for k3. Both need an independent challenge before k186 runs the
suite. The leaf cuts `review-impl` before k186.
