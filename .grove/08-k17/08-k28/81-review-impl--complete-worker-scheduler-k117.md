# complete-worker-scheduler-k117

**Reviews:** complete-worker-scheduler-k114

## Goal

Independently challenge the incremental workload termination signal/exit seam
against k114's bounded contract and the applicable termination timing spec.

## Context

Read the k114 producer and
experiments/exec-reliability/evidence/termination-turns-k114/report.md,
controller.py `_Termination`/`_terminate`, and the direct/real controls in
test_controller.py. Read docs/specs/diagnostic-process-inspection.md and the
termination-cutoff-k90 report for the original admission/reserve rules.
Read incremental-review-k115 for the worker-turn boundary and the limits of
passing evidence. k116 retains the entire aggregate charter/history; this
review must not certify shared scheduling, teardown overlap or guest acceptance.

## Done when

Trace constructor/advance/compatibility-consumer behavior. Challenge fixed
T/K/reserve, at most one exit observation per turn, nonblocking turns, absolute
wakes, starved callers and equality/overrun cutoffs, mandatory group KILL after
leader exit, PID-only scope, sticky errors, terminal idempotence, the sole final
inspection and signal epochs. Inspect the moved renewed-K mutation and the
three retained named regressions for a meaningful behavioral challenge rather
than a broken injection. Challenge real fixture identity, independent exact
reap/absence receipts, checker bounds and preservation/hash audit claims.

Account for all three full-run outcomes and the retained lost-result transport
sample alongside the five interleaved baseline/candidate pairs. Distinguish
source/mutation-anchor changes from unresolved transport repeatability. State
which claims hold, findings with source/evidence and disposition questions,
and material limits. Attempt a named regression within the review skill's
permitted work. Do not fix findings; reconciliation is earned by findings.

## Notes

No guests, privilege, installed changes or unrelated processes. Preserve every
sample. k116 owns compatibility-loop removal, shared earliest-deadline
scheduling, actual worker/teardown overlap, all remaining controls and final
bounded-inspection-k28 review. k28/k17 remain open; acceptance is 0/10,
unavailable. No empty integration leaf.
