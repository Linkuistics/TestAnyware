# group-kill-ordering-k33


## Goal
Prevent synchronous process-table inspection from delaying or suppressing the
controller's group KILL at the end of TERM grace.

## Context
This is an independently testable improvement to the existing controller, not
the complete bounded-worker guarantee. The parent and diagnostic inspection
spec remain binding on k30. Follow-up bounded-inspection-k34 owns the remaining
worker, scheduler, protocol, shutdown and complete control inventory.

## Done when
- The real controller makes no group query during TERM grace and attempts group
  KILL at fixed T even when the leader has exited. Keep the leader pinned until
  signals and probes finish. PID-only signalling remains conditional and narrow.
- Group K is fixed at T plus the configured kill allowance, including when exit
  observation overruns T. A later table cannot repair an earlier denied signal.
- Real process tests and regression mutations demonstrate ordering, sticky
  permission uncertainty, PID-only scope and fixed deadline behavior. Freeze a
  complete diagnostic self-check and independently audit recorded identities.
- Document the synchronous post-KILL limitation; preserve prior evidence and
  schedule review-impl group-kill-ordering before k34 as the final producer act.

## Notes

Delivered evidence: `experiments/exec-reliability/evidence/k33-final-02` passes
lint, format, strict types and all 75 tests. The report's k33 section explains
the red and intermediate failures, exact source snapshots and measured ordering.
`k33-audit-02` checks 1,345 retained condition artifacts and 1,393 audit inputs,
observes all 257 recorded PIDs absent, checks 77 resources and cleans 212 owned
fixture directories. Review the earlier failed audit too; these are later
observations, not escaped-child or hard helper-wait guarantees.

## Decisions (running log)

- Decomposed k30 because its complete worker/scheduler and adversarial inventory
  exceed one focused session. This first slice independently improves safety
  signal ordering; it does not claim bounded inspection after KILL.
- Executable RED controls contest the ordering claim. The scheduled review will
  supply fresh context; no competing in-session reviewer is needed.
- TERM EPERM stays permission_unknown without a contemporaneous table. A final
  zombie table cannot establish the cause of that earlier denial.
- The first full run exposed the real macOS zombie-only KILL EPERM path after
  denied TERM. The control now accepts either sent KILL or observed
  zombie_only_eperm, while requiring the earlier permission_unknown and overall
  failure. The intermediate run and its exact inputs are retained separately.
- Audit 01 conservatively stopped before cleanup on a recorded PID now shown as
  a macOS metadata worker. Preserve that failed observation without signalling
  or inferring reuse. Audit 02 retains it as an input and makes a separate later
  absence observation with the same refusal predicate; no audit was rewritten.
