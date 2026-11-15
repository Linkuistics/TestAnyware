# bounded-inspection-k147

**Reviews:** bounded-inspection-k28

## Goal

Review the complete bounded-inspection production contract before retained-evidence-k21 consumes its observations and residual inputs.

## Context

Read k28's brief, k17's promoted acceptance requirements and `docs/specs/diagnostic-process-inspection.md` in full.
Review the commit that retires complete-worker-scheduler-k146, including its current runtime and frozen source condition.
Use `experiments/exec-reliability/evidence/worker-overlap-k146/report.md` for the source, control and retained-failure inventory.
The report links both interleaved full comparisons, mutations, independent overlap audits, reusable checks and unavailable-control receipts.
Read earlier reports where the current contract relies on their controls or reconciliation.

Trace actual `execute` and all runtime ownership through `controller.py`, `worker_capture.py`, `inspection.py`, `turns.py`, `peer.py` and their callers.
Include preparation, foreground workloads, reverse resource cleanup, disposal, public absence commands and final close.
Review the existing full control inventory as well as the eight new production overlap cases.
The optional standalone driver interface alone cannot establish production scheduling.

## Done when

- Trace helper intent, fork admission/reset, gated identity publication, exec, output drains, late control parsing and exact reap.
  Distinguish explicit fork failure, unknown creation and independently retained helper obligations.
- Trace every parent wait, readiness callback, signal entry point, destructor and shutdown hook.
  Check workload and worker authority separately, including early reap, stale callbacks and cutoff closure.
  Check fixed T/K/R, mandatory group KILL, conditional PID KILL and the sole final query.
- Challenge production failure retirement while cleanup or disposal remains open.
  Check safety priority, channel caps, original worker deadlines, already-retiring close and request retirement across command transitions.
  Check preparation exits, healthy close, caller exceptions and residual ownership without private blocking cleanup.
- Independently check source identities, frozen subjects, artifact hashes, case identities, mutation rejection and exact fixture cleanup.
  Check each distinct worker, helper, workload leader and observed-member obligation.
  A worker reap cannot discharge another owner, and a damaged receipt stays failed.
- Reconcile historical timing failures and k145's unresolved interrupted live-group failure without retrospective correction claims.
  Check the actual setuid-ps KILL and genuine UID-denial unavailable receipts against the stated prerequisites.
  Missing real controls prevent a full guarantee, and guest acceptance remains 0/10 unavailable.
- Record findings against the complete k28 contract before k21 proceeds.
  Create reconciliation work only for actionable findings, with no empty integration.
  If runtime defeats the contract, externalise the precise counterexample to design before its consumer.
  Do not extend ceilings, add threads or workers, acquire privilege or widen signal scope.

## Consumer boundary

k21 owns stream and result identity, surviving writers and retained-file late changes.
k18 owns stalled evidence storage and teardown evidence descriptors.
Neither boundary permits publication acknowledgements to gate helper exec, workload signals, reap or disposal.
k17 remains open until its complete acceptance requirements hold after review reconciliation and retained-evidence work.
