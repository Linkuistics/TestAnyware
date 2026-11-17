# macos-exit-observation-instrumented-k195


## Goal

Separate a blocked guest `ps` from an exit that the macOS agent did not
observe, after an intentional timeout. Give the k4 inquiry its cause
statement.

## Context

- `macos-timeout-inspection-k193` took the first slice of the k4 remainder:
  the complete original r3 fixture condition on one fresh unchanged clone.
  This leaf owns the rest of that remainder. Read the k193 section of
  `docs/experiments/exec-reliability-macos-timeout-inspection.md` first.
- `docs/experiments/exec-reliability-macos-timeout-inspection.md` and the
  decision log of `macos-timeout-inspection-k4`. In run 02, `after-09-table`
  printed 460 rows that end with its own `ps` row, then timed out at 30 s.
  Only checker revision 2 counts that call, so it is a post-hoc expansion.
  Completion of `ps` or the shell, the action that ended PID 630 and the
  agent's wait phases stay unproved until instrumentation measures them.
- The k4 report names three candidates. The first is the k3
  completed-then-timed-out mechanism, with detector
  `diagnostic:macos-completion-old-path`. In it, `waitForExit` waits on a
  second global-queue block that can starve. The second is a `ps` exit delay
  on a process in state U. The third explains the 33 s against 30 s shapes.
- Checker revision 3 of `macos-timeout-inspection-k194` applies. Only a
  valid own observation proves absence. A stall needs valid local evidence
  after a valid intentional timeout. Owned cleanup ends
  `cleanup confirmed` or `cleanup uncertain`.

## Done when

- One instrumented clone records, for each post-timeout guest command, when
  the shell and its children exit and when the agent observes the exit.
  Deterministic process controls force the suspected orderings. The observer
  and its teardown are bounded.
- The report states which candidate the evidence establishes, or states that
  the cause stays unproved. A failed or timed-out inspection never proves
  absence.
- Owned children and the clone are accounted for and disposed even on
  failure, with residual identities and remedies under the common root
  contract.

## Notes

Select no production ownership or cancellation mechanism. No broad process
name cleanup. The instrumented clone is the one isolated instrumented clone
that the root brief allows for this causal inquiry. k4 run 02 is its
unchanged clone for the issue payloads, and the k193 clone is its unchanged
clone for the r3 condition. Instrumented runs use checkout-built tools and
owned disposable resources only. If the isolated build or route is not
available, the condition stays unavailable.

## Decisions (running log)

- The seam is `agents/macos/Sources/TestAnywareAgent/ExitTrace.swift`
  behind the `EXIT_TRACE` compile flag. It records each `runShell` wait
  phase and the kernel fork, exec and exit of the shell and its children
  through kqueue `EVFILT_PROC`. The default release agent holds no
  `ExitTrace` symbol, so no production behaviour changes.
- Host controls are frozen in
  `experiments/exec-reliability/evidence/macos-exit-observation-k195/controls-01`
  from commit `dbf23c70`. All four controls pass in each of three runs. Task
  `diagnostic:macos-exit-observation-controls` reproduces them.
- The controls show both candidate orderings on the host. In a starved
  global queue, the shell exits at 2 ms and the agent reports a timeout at
  the 2 s deadline. An ignored TERM adds the 3 s TERM wait before SIGKILL.
  That wait matches the difference between the 33 s and 30 s shapes. The
  host controls do not show which ordering happened in the guest.
- The guest run needs the isolated route of the root brief: a
  checkout-built app, CLI and traced agent in one owned clone. The app
  handover needs the owner's interval and Local Network grant. No record
  gives that authorisation, so the leaf asks the owner.
