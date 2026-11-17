# macos-exit-observation-instrumented-k199

**Integrates:** macos-exit-observation-trace-k198

## Goal

Triage the committed review of `macos-exit-observation-instrumented-k195`
and apply the findings that meet the agreed acceptance and material-risk bar
before subsequent implementation relies on the diagnostic.

## Context

Read the findings from the commit for `macos-exit-observation-trace-k198`,
then verify each against the producer's source, retained evidence and the
inherited root contract. The review's commit owns the finding list.

## Done when

Each finding has a supported disposition. Necessary bounded repairs and
appropriate verification are complete, with the report and retained evidence
distinguished from any new verification. Any accepted trade-off is explicit
against the agreed acceptance bar.

## Notes

This is the next flat sibling after the review so its evidence coordinates
remain current. Preserve frozen runs and do not substitute a fresh run for
adverse retained evidence. The root's resource and authorization boundaries
continue to apply.

## Decisions (running log)

- Disposition of R1, the kernel time and the deadline proof: accepted and repaired.
  A kernel record time is the time the trace thread received the event batch. So it
  bounds the exit only from above. The checker now puts the deadline at `wait-enter`
  plus the requested timeout. It calls a call `command-alive` only when the shell died
  by SIGTERM or SIGKILL after `terminate-sent`. Any other late or absent exit is
  `uncertain`. The verdict `starved` needs the sequence and the clock to agree.
  `emit` now takes the time under the lock, and the seam comment names one clock.
- Disposition of R2, registration and PID reuse: accepted and repaired. Each
  registration carries a token in `udata` and a process start time. A failed
  registration is undone. A full child snapshot emits `adopt-truncated`. The checker
  reports such a call as a descendant lower bound. One residual stays: a reused PID
  whose old exit the thread has not drained is not traced. The report states that gap.
- Disposition of R3, completeness: accepted and repaired. Evidence is complete only
  when each receipt is present and clean and each intentional timeout timed out. The
  trace must hold one `trace-open` from the verified agent and a contiguous sequence.
  It must hold no unparsed line, no loop error and no unmatched `begin`. The checker
  exits 0 only for a traced, complete run with proved cleanup. New tests feed damaged
  evidence.
- Disposition of R4, the report: accepted and repaired. The host control table now
  gives the measured ranges of `controls-01`. The host lag is 62 ms to 64 ms. The
  default build claim names only what the controls measured.
- Verification: the repaired checker `k199-1` judged the frozen `run-01` again. The
  verdict did not change: route `traced`, evidence `complete`, stall `not-reproduced`.
  The repaired seam ran the host controls into the new directory `controls-02`.
  `controls-01` and `run-01` stay frozen. This leaf made no guest run.
- Swift tests: the default suite does not compile `ExitTrace.swift`. The first full
  run hung for 2 h in `OwnedProcessFixtureTests.testRefusesProcessThatStartedBeforeTheFixture`.
  Its `waitUntilExit` waited on the main thread after the child was gone. A second run
  skipped that case. In that run, `testDescendantHoldingFDDoesNotWedgeAfterParentExits`
  timed out once at a host load of 13 to 17. It then passed three times alone. The
  skipped case passed alone in 1.2 s. The 47 Swift Testing tests passed. Both faults
  are intermittent and predate this leaf. They are outside the scope of this leaf.
