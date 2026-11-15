# complete-worker-scheduler-k115

**Integrates:** complete-worker-scheduler-k113

## Goal

Triage the findings of complete-worker-scheduler-k113 against the k110
implementation and apply the real ones.

## Context

Read the findings from the k113 commit and its leaf file. Grade each against
the source and the spec. The k110 slice stays narrow: k114 owns the shared
scheduler, cleanup overlap and the final k28 review.

## Done when

Each finding has a classification (unclear contract, real issue, visible
trade-off or noise) with evidence. Real fixes keep every prior sample and
receipt unchanged.

## Notes

No guests, privilege, installed changes or unrelated processes.

## Decisions (running log)

- F1 — real issue in the control: the incremental overrun case disables the
  sleep guard around the entire turn. Capture the original sleep for the
  explicitly delayed waitpid seam and guard every begin/advance call. Retain a
  drain-loop mutation showing that the overrun control rejects caller blocking.
- F2 — visible trade-off with an unclear late-caller contract: the spec fixes
  invocation phases and fails measured overruns (spec, Worker failure and close).
  Starting a new TERM grace on a late first turn would violate those cutoffs.
  Keep immediate overdue escalation, document it, and control first turns in
  grace, at/after KILL, and at/after the final cutoff without renewing authority.
- F3 — real issue: closed means shutdown begun, while residual_remedy currently
  claims authority is closed before completion. Only emit that terminal remedy
  after shutdown_finished; the existing started/finished/shutdown fields expose
  in-progress state. Cover it before changing production evidence.
- F4 — real control gap: the compatibility-driver clock tests do not check the
  incremental API's exact wake. Add direct fake-clock turns with real deadline
  and owner guards at grace/KILL/final boundaries; no OS child is claimed.
- F5 — real unresolved repeatability issue, already retained for k114. The
  unsupported ps row precedes close, but that does not establish its cause.
  Declare ten isolated ordering/ignore-term samples on the parent and ten on
  k110, preserve all outcomes, and report the comparison without erasing the
  failed full run or claiming aggregate acceptance.
- F5 comparison completed: ten isolated parent (1e8cbf4d) samples and ten k110
  (cbd2aace) samples all passed. Inputs were reconstructed byte-for-byte from
  those revisions before running the declared fixed count. This does not
  reproduce or explain the earlier unsupported ps state; it remains unresolved
  for k114. No parser or timing allowance changed.
- F3 red control reproduced the premature remedy. The first clock-control
  fixture used decimal durations and failed on binary float representation;
  retain that run, then use exactly representable 0.125/0.25 phase budgets.
  Corrected red: 1 expected failure, 28 passes. The production repair gates the
  remedy on shutdown_finished. F2's immutable-deadline trade-off is now explicit
  in the spec and controlled through actual owner guards with simulated OS calls.
- Final validation: focused 117/117, full 826/826 plus lint/format/strict types,
  review controls 238/238. Drain-loop mutation rejected 3/3 delayed-wait turns;
  KILL-wake and final-wake mutations produce the expected four/eight failures.
  The audit verifies 28 conditions, 3,219 subjects and 224 named child cleanup
  receipts; every checker was reaped without timeout. All 172,739 prior evidence
  paths remain unchanged. Evidence, reproduction and limitations are retained
  in experiments/exec-reliability/evidence/incremental-review-k115/report.md.
  k114 receives the handoff and stays live; no aggregate acceptance or ancestor
  closure is claimed.
