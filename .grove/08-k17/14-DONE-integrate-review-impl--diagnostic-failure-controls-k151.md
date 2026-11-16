# diagnostic-failure-controls-k151

**Integrates:** diagnostic-failure-controls-k150

## Goal

Triage the k150 review against the k149 artifact and the agreed k17 contract,
apply warranted repairs and reconcile the resulting acceptance evidence.

## Context

Read the findings from the commit that retires diagnostic-failure-controls-k150.
Read its producer diagnostic-failure-controls-k149, including the full done-when
list and decision log. The durable handoff is
`docs/experiments/exec-reliability-diagnostic-foundation.md`; the reviewed
controls and original-receipt pointers are in
`experiments/exec-reliability/evidence/failure-controls-k149`.

## Done when

- Each review finding is verified against its cited source and retained evidence
  and explicitly classified under the integration discipline.
- Warranted changes have appropriate frozen failing/passing controls and
  itemized resource evidence in fresh directories, with original receipts kept.
- The diagnostic handoff and k17 whole-contract assessment reflect the resulting
  evidence and its limits. Completion does not rest on passing later samples
  alone or an unavailable prerequisite.
- Any newly necessary independent review is concrete and earned by the changes;
  no unchanged aggregate continuation replaces the acceptance attempt.

## Notes

k17 stays open through this integration. k18 owns stalled storage. No live guest,
app or ten-clone acceptance claim follows. The real UID-denial fixture needs a
separately authorised human owner; do not recreate it on inferred permission.

## Findings

- R1 is a real issue. The k145 and k132 peers printed the child PID, so the
  never-ready control does not explain those runs. The k151 verdicts keep the
  old-peer defect and record the attribution as unresolved.
- R2 is a real issue. The k151 readiness tool needs an identity and a clean `ps`
  result for "absent". Otherwise it reports "unknown". `absence_controls.py`
  fails the k149 predicate in three cases and passes the k151 verdict in five.
- R3 is a real issue. The k151 verdicts record the five groups as
  `bound_exhausted_cause_unknown` and keep the part of each mechanism that a
  control demonstrates.
- The stale k110 mechanism phrase now matches the runtime comment.

The k17 whole contract stays unmet. The human chose an instrumented soak over
accepting the six cases as visible limits, so historical-failure-soak-k152 owns
it. The evidence is in
`experiments/exec-reliability/evidence/failure-controls-k151`.
