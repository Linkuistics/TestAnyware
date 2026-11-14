# bounded-inspection-k34 — brief


## Goal
Complete the bounded inspection worker and shared controller scheduler required
by bounded-inspection-k30 and its parent bounded-inspection-k28.

## Context
Read both parent briefs and docs/specs/diagnostic-process-inspection.md in full.
The preceding group-kill-ordering-k33 producer and review improve only TERM
grace ordering in the synchronous controller. Read their artifacts and limits;
they do not satisfy the worker or complete deadline guarantee. Initial fork
admission k29/k32 remains only admission. The earlier draft is untrusted.

## Done when
- Deliver all remaining parent requirements and its complete real-controller
  control inventory: private helper-owning worker, shared scheduler, bounded
  protocol, fixed deadlines, sticky failure, shutdown concurrent with teardown,
  explicit residuals and independent exact identity audits.
- Reconcile k33 with the final scheduler while retaining its safety properties.
- Extend Taskfile and evidence subjects as necessary, freeze new evidence and
  update the report without rewriting prior runs or claiming guest acceptance.
- As the final producer act insert review-impl bounded-inspection before
  retained-evidence-k21 with **Reviews:** bounded-inspection-k30, requiring the
  runtime ownership, waits/destructors/concurrency and full inventory audit in
  the parent. Do not schedule an empty integration.

## Notes
