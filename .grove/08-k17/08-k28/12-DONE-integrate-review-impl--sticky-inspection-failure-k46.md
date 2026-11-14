# sticky-inspection-failure-k46

**Reviews:** sticky-inspection-failure-k45

## Goal
Reconcile the findings of review k45 on the sticky-inspection-failure slice before the worker/scheduler implementation builds on it.

## Context
The findings are in the Decisions log of k45, anchored to commit 35d806d. Re-read the cited code before changing it, because positions may have moved.

## Done when
- Each k45 finding is fixed, or rejected with a reason in this leaf's decisions.
- Unobserved leader exit and an exhausted cutoff stay distinct from a failed inspector query in the retained receipts.
- A control exercises a real failed query and checks that later calls stay unobserved.

## Notes
Scope is this slice only. k44 keeps the full worker/scheduler contract.

## Decisions (running log)

- k45 finding 1 — actionable, fixed. `_final_snapshot` returns synthetic unknown
  observations for missing leader exit and exhausted cutoff without querying.
  `GroupObservation.query_attempted` now remains false until helper invocation;
  `_InspectionState.snapshot` latches only an attempted query's unknown result.
  Both skip reasons retain distinct errors and fail the affected call without
  disabling later safety inspection. Remaining workload stays unreached.
  Added both later-safety controls and strengthened the existing real withheld
  exit controls for both signal scopes. All four failed on the old latch with
  populated `inspection_failure` (not missing schema) in k46-red-01.
- k45 finding 2 — actionable coverage gap, fixed. A query argv adapter executes
  real `/bin/ps -o` with missing format on the workload query. Raw usage stderr
  and nonzero status pass through the real parser. Later safety commands still
  run and reap, but queries and probes stop; disposal certainty remains failed
  while the actual marker is removed. This control passes before and after the
  repair. It establishes real helper failure, not spontaneous corruption or
  genuine UID denial. Existing injected-failure controls remain separate.
- k45 advisory finding — accepted reporting correction. Original k43-previous-02
  XML shows `KeyError: 'inspection_failure'` for the valid-live control; its
  retained audit has all four queries and its result says disposed. No behavior
  improvement is claimed from that failure. The control now checks continuation
  before a schema-neutral lookup. The report records the correction; historical
  evidence is unchanged.
- Verification: k46-final-01 passes 119 tests, Ruff lint/format and strict Pyright;
  read-subject hashes match and all checkers reap without timeout. Frozen source
  snapshots are in k46-inputs-01. Only controller.py and test_controller.py differ
  before/after; the latter adds two new-field assertions to the real-helper
  control after the red run. The four red regressions are unchanged.
  Executable failing regressions discharge implementation doubt; no optional
  integration reviewer was needed for this seven-line production change.
- Independent k46-audit-01 passes: 300 recorded real PIDs absent, simulated PID
  excluded, 72 resource markers and 92 argv-derived scratch directories absent
  after allowlisted cleanup. Its absence oracle is contested with live, mixed,
  invalid-selector and freshly reaped controls. An initial lint-only rejection
  was corrected before the audit body ran. Source and artifact hashes stay equal.
- Handoff: k44 remains live, so k42 does not close and no parent cascade occurs.
  The parent brief and durable report carry this refinement. No new ADR is
  required for this correction within the existing inspection contract. Full
  worker/scheduler, protocol, shutdown, concurrency and fault-inventory work is
  still required; historical guest acceptance remains 0/10, unavailable.
