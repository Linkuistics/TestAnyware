# storage-controls-k170

**Reviews:** storage-controls-k163

## Goal

Review the nine storage controls adversarially before the writer slices rely on
them as their acceptance gates.

## Context

Read the producer commit for `storage-controls-k163`. The controls are in
`experiments/exec-reliability/test_storage_controls.py`. The task
`diagnostic:storage-controls` runs them. The producer leaf records the observed
failure of each control and its cause. The spec is
`docs/specs/diagnostic-evidence-storage.md`, in "Test seams and required
controls".

On this build no writer exists. Each fault fixture wraps the storage functions
that the controller calls today. A stall is a FIFO open that a signal
watchdog interrupts. Two adapters, `storage_outcome` and
`writer_descriptors`, return `None` until a later slice binds them.

## Done when

- Check that each control asserts the final behaviour of the spec and not a
  property of the fixture. A control that passes for the wrong reason after the
  repair is a finding.
- Check that each recorded failure comes after the fault. It must be a
  behavioural assertion or a watchdog expiry, not a setup error.
- Check that the marker of each control names the slice that the node brief
  gives it.
- Check that the fixtures can switch to the writer wrapper without a change to
  the assertions. Name any assertion that a later slice must change.
- Check the watchdog bounds against the phase bounds that the controls declare.
- Check that a stalled control leaks no resource marker or process.

## Notes

Four controls use watchdog expiry as their recorded failure. They are stalled
writer, blocking storage call, outbox overflow and intent gate. Readiness
failure also uses it, with a shorter bound. Judge whether a watchdog expiry is
specific enough to show the gap that each marker names.

The no-controller-storage-calls control uses an audit hook that stays
installed for the rest of the process. It records only while a log is set.

## Decisions (running log)

The review found acceptance gaps in the controls' observations, rather than a
need to implement the writer early. Repair belongs in an adjacent
`integrate-review-impl` sibling before `storage-writer-k164`, so later slices
can rely on the corrected controls. The review changes no production or test
code and runs no test, build, lint or format command.

## Review findings

Reviewed producer commit `04e9c6c141d7d6fcaa13f2b2d5eeb5fb463defae`
(`storage-controls-k163`) against the nine required controls in
`docs/specs/diagnostic-evidence-storage.md`. These findings concern agreed
acceptance, not optional test polish.

### F1 — P2: final absence does not prove forward progress stopped

At `experiments/exec-reliability/test_storage_controls.py:378`,
`experiments/exec-reliability/test_storage_controls.py:395` and
`experiments/exec-reliability/test_storage_controls.py:459`, the storage-error,
writer-death and intent-gate controls check the second resource only after
`execute()` has returned. Their oracle is `not owned(tmp_path, second)`.
The peer's acquire creates that marker and its dispose removes it
(`experiments/exec-reliability/peer.py:170`). A controller that wrongly acquires
the second resource and then disposes it satisfies exactly the same assertion
as one that never acquires it. The terminal failure code does not distinguish
those histories either.

The spec requires sticky forward stop and, for intent-2, that the second
resource is never acquired. Observe acquisition attempts or retain an
independent acquisition history that disposal cannot erase, and assert the
ordering against the failed gate or observed storage failure. These final
assertions need strengthening before switching to the real writer fixture;
changing only the fault fixture and outcome adapters cannot close this gap.

### F2 — P2: unchanged teardown is only partially observed

The stalled-writer control at
`experiments/exec-reliability/test_storage_controls.py:312` checks final
resource absence and that `_InspectionState.close()` returned. Storage error,
writer death and intent gate similarly check marker absence. None observes
whether cleanup ran: the fixture's cleanup is a `bytes` call, while disposal
alone removes the marker
(`experiments/exec-reliability/test_storage_controls.py:257`). Skipping cleanup
can therefore satisfy these assertions. Inspection can also be skipped while
final marker absence still holds. A returned worker-close method alone does
not establish its required termination/reap outcome.

The blocking-storage-call control at
`experiments/exec-reliability/test_storage_controls.py:334` stalls before
intent-1 and never acquires the first resource. Its final absence check is
already true before execution, so it provides no evidence that the independent
safety actions survived the blocking storage call. Yet the spec requires its
result to match the stalled-writer control.

Establish the reached fault/ownership state and observe the relevant cleanup,
disposal, inspection and worker shutdown outcomes independently of storage
publication. These observations must distinguish a skipped action from a
successful one. Current post-run assertions cannot remain unchanged as promised
and still establish unchanged teardown.

### F3 — P2: whole-run watchdogs do not verify the declared phase bounds

`assert_killed_publication()` at
`experiments/exec-reliability/test_storage_controls.py:303` checks only total
elapsed time against the 12-second watchdog. The stalled-writer control records
a worker-close timestamp but checks only that the list is nonempty. There is
no comparison for cleanup/disposal ceilings, worker-close bounds, KILL at the
publication deadline, or reap by that deadline plus two seconds. A run can
delay a safety action or spend several seconds in publication and still finish
under 12 seconds with `publication == "killed"` and `writer_reaped == True`.

Readiness has the same gap at
`experiments/exec-reliability/test_storage_controls.py:418`: its declared
readiness bound is one second and reap grace is two seconds, but the sole time
assertion allows five seconds for the entire invocation. It does not establish
reap within two seconds of the readiness deadline. The half-second intent gate
is likewise unobserved within the 12-second run allowance.

Keep an outer watchdog for suite containment, and separately assert phase
start/deadline/completion observations and safety action ceilings, with an
explicit scheduling tolerance. The short declared bounds must determine those
assertions. A fixture replacement alone cannot make the existing assertions
prove these timing requirements.

## Inspection evidence and limits

The nine marker owners match the producer's removal table. The source paths
support the recorded first failures: five watchdog expiries after injected
stalls, marker-absence failures after permission/EPIPE faults, controller audit
events, and regular stdout/stderr descriptors. The outcome and writer-report
adapters are deliberately unbound on this build; their current `None` results
are not findings. The audit window is explicitly reached and disabled in a
`finally` block. The inherited-state assertions cover both call and writer
descriptor reports once its adapter is bound.

The recorded counts (9 expected failures; 975 passing tests in the full suite)
are producer evidence, not independently reproduced results. No raw per-control
run output was linked. Source inspection supports the stated failure paths but
does not certify a leak-free measured run. In particular, the producer itself
records that the error/death cases leave their first resource marker acquired;
the broad statement that no control leaks a marker does not cover those cases.
For the five current stalled paths, watchdog expiry releases the injected fault
before unwinding through teardown. The watchdog then remains disarmed during
that unwind, so its expiry alone is not evidence of bounded final teardown or
reap. The real-writer binding must retain explicit cleanup on test failure.

Graph evidence was insufficient for these new/current files. The project was
confirmed as
`Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`.
Coverage generation `2026-11-15T02:19:52Z` reported the test as not tracked and
the controller, peer and task/config files as stale. Direct source and committed
diff inspection supplied the evidence above; no graph completeness claim is
made. Only the review task and its integration handoff are changed.
