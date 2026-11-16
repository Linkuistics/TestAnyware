# macos-process-fixtures-k189

**Integrates:** macos-process-fixtures-review-k188


## Goal

Triage the committed review of macos-process-fixtures-k185 and apply the
findings that meet the agreed acceptance and material-risk threshold, before
macos-completion-live-k186 relies on the fixtures on the shared host.



## Context

Read the findings from the commit naming macos-process-fixtures-review-k188,
then verify them against the current implementation and the root brief.
The producer is macos-process-fixtures-k185. Its implementation, report and
retained control runs are linked from the review and producer tasks.

This is a flat sibling because it shares the root brief and directly unblocks
the next macOS experiment. Findings remain in the review rather than becoming
this task's repair checklist.

## Done when

- Each finding is verified and classified, with the reason for accepting or
  rejecting it recorded against the agreed bar.
- Necessary bounded repairs and their verification are complete, with evidence
  showing what the next live macOS leaf can safely rely on.
- The original acceptance and adverse evidence remain visible; any remaining
  work has an explicit owner rather than being silently dropped.

## Notes

### Triage (running log)

I read the five findings from review commit `391e81a633` and checked each
against the cited source lines. Each failure sequence holds in the source. The
bar is whether k186 can rely on the fixtures and the control on the shared
host.

- R1 (High). Real issue. `track` adopts whatever process holds the PID, and it
  appends that process before the command check. On a shared host, k186 runs
  beside other processes, so an adoption of an unrelated process is a material
  risk. Repair: a child is accepted only in the session of the test process,
  after fixture creation, with an identity that stays the same across
  observations. A process that fails the binding is refused and never tracked.
  Measured on this host: Foundation `Process` gives each child its own process
  group, so a group check cannot bind ownership. The session is inherited.
- R2 (High). Real issue. The broad fault arm scans once, before a build of up
  to 900 s, and a failed `pgrep` counts as an empty selection. k186 does not
  need the fault arm, and its 20 retained runs stay valid as recorded. Repair:
  a failed scan stops the run, every run refuses a visible match, and the
  broad arm runs only with an explicit `--isolated-host` declaration. The
  scan-to-signal race remains. Only an isolated host closes it, so the report
  and the Taskfile state that limit.
- R3 (Medium). Real issue. A run counts as survival evidence without the
  fixture tests. Repair: a run needs a positive run count, no outer-bound
  stop and both fixture cases finished. In the owned arm, it also needs a
  cleanup receipt from each case with no survivor. Otherwise the verdict is
  `invalid-exercise`.
- R4 (Medium). Real issue. A child started before `track` binds it is outside
  teardown. Repair: `pidFile` registers the name, `track` records a child as
  soon as the binding holds, and `cleanUp` resolves each registered name that
  `track` did not bind. When `cleanUp` cannot resolve a name, the test fails
  with that evidence.
- R5 (Medium). Real issue. An exception after the control starts leaves
  `sleep 300` running. Repair: bounded stop and reap of the control on every
  exit after the control starts. The original error and any cleanup failure both stay in
  the record.

### Repairs and evidence (running log)

- R1 and R4 repaired in
  `agents/macos/Tests/TestAnywareAgentTests/OwnedProcessFixture.swift`.
  New tests in `OwnedProcessFixtureTests.swift` cover each refusal and the
  cancellation path.
- While I wrote the refusal test, I found a sharper form of R1. When a test
  runs early, xctest starts less than 1 s before the fixture. The xctest
  process therefore passes the clock allowance and shares the session. A PID
  file that named it made the fixture track xctest and send SIGTERM to it at
  cleanup. The xctest process ended with no result line. The binding now
  refuses the test process and every ancestor, because a child of the test is
  never one of them. `testRefusesTheTestProcess` holds that case.
- My first session test was invalid. Foundation makes each child a process
  group leader, and `setsid()` returns -1 for a group leader. The control
  stayed in the test session, and the fixture correctly bound it. The test now
  forks before `setsid()` and asserts that the session differs.
- `swift test --filter 'OwnedProcessFixtureTests|ProcessRunnerTests'`: 10
  tests, 0 failures, and no `sleep 30` remained afterwards. In an earlier run
  of the same filter, `testHandlesLargeOutputWithoutPipeDeadlock` failed with
  exit -1 after 10 s. That is the known intermittent failure that k185 and k188
  recorded. It is not caused by this change.

### Outcome

All five findings are real issues, and k189 repairs each one. No finding was
an unclear contract, a trade-off or noise. k185's acceptance and its 40
retained runs stay as they were recorded. All 40 runs pass the new exercise
check.

k186 can rely on these points on the shared host:

- The fixture signals only a bound child of the test. It never signals
  xctest, an ancestor, a process from another session or an older process.
- Cancellation of `track` cannot leave a child outside teardown.
- The owned arm counts a run only when the fixture cases ran and cleaned up.
- The control never outlives the script, and a failed scan stops the script.
- The broad arm does not run on a shared host.

Remaining work and owners:

- PID reuse between check and `kill`, or before `track` reads the PID, is a
  recorded limit with no Darwin repair. The report states it. No leaf owns it.
- The intermittent completed-then-timed-out failures of `agent:macos:test`
  belong to k186 (live run) and k4 (timeout inspection), as k185 recorded.
- A new broad fault-arm run needs an isolated host. No current leaf needs one.
