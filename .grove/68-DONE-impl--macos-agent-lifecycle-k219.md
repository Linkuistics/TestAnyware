# macos-agent-lifecycle-k219

## Goal

The macOS agent owns the lifecycle of each guest command through
`posix_spawn` and kqueue. It returns the typed exec record.

This leaf is the first part of candidate C. Leaf k221 owns the rest of C.

## Context

- Spec: sections "Exec record", "Bounds", "Compatibility", "Test seams and
  required controls" and candidate C.
- ADR-0023 and ADR-0024.
- Agent code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift` and
  `agents/macos/Sources/testanyware-agent/AgentServer.swift`.
- Old failing paths: `diagnostic:macos-completion-old-path` and
  `diagnostic:macos-exec-outcomes-old-path`.
- Retired leaves 17 to 39 hold the macOS causal evidence.

## Done when

- The exec types move into the vendored `TestAnywareAgentProtocol` module. An
  encoding drift test guards them.
- The owner meets the same lifecycle rules as leaf k217. That covers the group,
  the held leader, the grace, the kill-and-reap bound, the tombstone order,
  the identity checks and the capture deadline.
- The exec reply carries the typed record. The legacy fields keep the macOS
  values of the old agent.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference.
  - Old-fail: a child that ignores SIGTERM receives SIGKILL after the grace.
    The old path is the term-ignored case of
    `diagnostic:macos-exec-outcomes-old-path`.
  - New behaviour: the leader status is unavailable at K+1.4 s.
  - New behaviour: the capture is large or stalls.
  - Old-fail: `diagnostic:macos-exec-outcomes` shows the exact bytes, exit
    23, signal death and the spawn errno. The old path is
    `diagnostic:macos-exec-outcomes-old-path`.
  - Old-fail: `diagnostic:macos-completion-old-path` at `free-1` fails on the
    old runner. The same task passes with `EXPECT=clean` on the new runner.
  - Compatibility: the legacy projection equals the recorded replies of
    today's macOS agent.
- `agent:macos:test` passes 20 consecutive times.
- Add the new macOS replies to the reader tests of leaf k218.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The cause of the original macOS post-timeout stall is unresolved. No part of
this leaf may depend on it. Keep the retained samples, and report a
recurrence as a failure. Cancellation and the single-handle download belong to
leaf k221.

## Decisions (running log)

- D1. The owner lives in `ProcessRunner.swift` and compiles alone, with Foundation, Darwin and CryptoKit only. The k3 and k12 detectors compile that one file with `swiftc`, so a dependency on another file would break the old-path tasks. The wire types live in `TestAnywareAgentProtocol/ExecRecord.swift`. A mapper in the `TestAnywareAgent` module copies the owner result into them.
- D2. Each exec gets one dedicated owner thread. The thread blocks in `kevent` on its own kqueue, with an `EVFILT_PROC` `NOTE_EXIT` source and the `kevent` timeout as the timer. No GCD pool worker waits for the command, so the k3 pool-saturation stall cannot occur. One continuation resume returns the result to the caller.
- D3. The spawn uses `POSIX_SPAWN_SETSID`, `POSIX_SPAWN_CLOEXEC_DEFAULT`, `POSIX_SPAWN_SETSIGDEF` for every signal and an empty mask. `CLOEXEC_DEFAULT` keeps the agent sockets out of the command. The signal reset keeps the old Foundation behaviour for traps.
- D4. The capture directory is `testanyware-exec-<UUID>` in the per-user temporary directory, mode 0700. The prefix keeps the k3 capture audit meaningful. The owner opens the read descriptors before the spawn, and the worker closes them, so an abandoned worker never reads a reused descriptor.
- D5. As in k217, every record key is always present, and an inapplicable value is null. `reason` appears only on an incomplete stream. A stream with `bytes` equal to `fileBytes` at the deadline is complete, so an empty stream is never incomplete.
- D6. The seams replace only the leader waiter and the capture read, as in k217. The unavailable-status seam lies about the exit but reaps through the real `waitpid`, so the late reap still runs.
- D7. The legacy projection keeps the old Foundation decode and the old `\s+$` regular expression, unchanged. The control is equality with the recorded replies of the old agent, not a reimplementation.
- D8. A request may carry `id`. An invalid `id` gets HTTP 400 `exec_failed`, as in k217. The duplicate-id registry and HTTP 409 belong to k221. An invalid `timeout` keeps its old behaviour.
- D9. The spawn-failure test runs in a fresh process. The test starts its own `xctest` binary again through `/bin/bash -c 'ulimit -Su 1; exec ...'` with a marker variable, so the limit never touches the main test process.
- D10. The fixture self-test controls reap by exact PID. At the base commit 27053381d8, `OwnedProcessFixtureTests` hung in 5 of 12 runs. `Process.waitUntilExit()` blocked after Foundation had reaped the Perl control, so no process remained. This blocks done-when 5. The controls now use `posix_spawn` with an empty signal mask and default dispositions, and `waitpid` on the exact PID. The Perl control also writes its PID file only after the child runs `sleep`, through a close-on-exec pipe. The base had that race too: it failed 3 of 15 runs once the hang was gone. Result: 20 of 20 clean runs. The report records the Foundation wait as evidence for the open "waitUntilExit missed the exit" hypothesis.
- D11. The fixture adopts an exec session. The owner starts each command in a new session, so a child that the command leaves never runs in the test session. `OwnedProcessFixture.adopt(session:)` accepts the session that the owner's `leaderPid` names. The other binding checks stay: start after the fixture, a PID file, and a stable identity. `testDescendantHoldingFDDoesNotWedgeAfterParentExits` now calls `runOwned`, checks `remainingGroup` is `present`, and asserts on the legacy fields.
- D12. The errno names come from a table, not from interpolation. Interpolating a `POSIXErrorCode` gives `POSIXErrorCode(rawValue: 35)`, so `testSpawnFailureIsAnOutcome` failed. The table equals errno 1 to 107 of the macOS 27 SDK `sys/errno.h`, and a script compared it entry by entry. The signal table equals signals 1 to 31 of `sys/signal.h` by the same check.
- D13. The macOS replies are recorded at the owner seam, through the exec mapper and the plain `JSONEncoder` of `jsonOK`, which is the `/exec` wire body. `task agent:macos:record-exec-replies` writes them to `agents/macos/Tests/fixtures/exec-records`. The HTTP seam of the new agent is covered by `diagnostic:macos-exec-outcomes`, which runs a loopback copy of the agent. The fixtures hold the eight legacy macOS cases, the k217 extra cases and `trim-unicode` for review k231 finding R3.
- D14. The encoding drift test is a round trip: each recorded body decodes into `ExecReply` and encodes back to an equal JSON value. The Rust reader reads the same files, so both sides of the wire check one set. Controls seen failing: an `encodeIfPresent` mutation of `durationMs` failed the round trip, and a changed legacy `stdout` failed the Rust reader test. Both subjects were restored and compared byte for byte.
- D15. The k12 checker gains two record seams, `http-record` and `cli-json-record`, and the mode `--expect-record` (task `diagnostic:macos-exec-outcomes-record`). They read `outcome`, the evidence bytes and the termination block in the raw HTTP bodies and the Host CLI JSON output that the run already keeps. The legacy seams keep today's defects by design, so `--expect-defects` alone cannot show the repair. The run `outcomes-01` on the new owner gave check exit 0 and old-path exit 201, with only `grace-exceeded` missing. Its record judge gave exit 0, with all 16 verdicts faithful. Retained `run-01`, built from the base runner `6a3c3846…`: old-path exit 0, record exit 201 (`record-missing`). The judge ran on a copy, so the committed `run-01/check.json` is unchanged. `judgements.txt` in the evidence directory keeps each output.
- D16. The k3 control ran on both runners with the unchanged task. A temporary jj workspace at the base commit built the base runner `6a3c3846…`. It gave verdict `defect`: each `free-1` trial completed and was reported as timed out after 6.8 s (`completion-base-defect-01`). The new owner `d4d7d783…` gave verdict `clean` with `EXPECT=clean`: each `free-1` trial was prompt, near 0.74 s (`completion-new-clean-01`). The retained k3 runs compiled `f9401669…`, an earlier runner, so they do not stand for the base runner.
- D17. The group probe after SIGKILL ends early only at ESRCH. The first 20-run loop failed 3 of 20: `testDeadlineTerminatesTheGroupWithinBounds` (twice) and `testIgnoredTermGetsKillAfterGrace` gave `groupEmpty: unknown`, because the owner stopped at the first EPERM. A C probe of the same group shape saw EPERM first in 11 of 60 trials, and ESRCH followed within 1.3 ms in all 60. A second run, kept in `probe-eperm/probe-60.txt`, saw EPERM first in 25 of 60, and ESRCH followed in all 60. The owner now keeps probing to K + 1.4 s, and the last probe decides: ESRCH `yes`, a live member `no`, EPERM `unknown`. The single probe after a normal exit stays one probe, as the spec states. The failed loop stays in `agent-test-20-first`.
- D18. The final owner is `a562fc62…`. The script `final.sh` measured it in sequence. The k12 run `outcomes-02` gave check exit 0, old-path exit 201 (only `grace-exceeded` missing) and record exit 0, as `judgements-02.txt` records. The k3 run `completion-new-clean-02` gave verdict `clean`. `agent:macos:test` passed 20 of 20 runs in `agent-test-20`, with equal subject digests before and after. The runs `outcomes-01`, `completion-new-clean-01` and `agent-test-20-first` measured the earlier owner `d4d7d783…`, and they stay as history.
