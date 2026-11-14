# worker-scheduler-k38

## Goal
Complete all remaining bounded-inspection-k34 requirements after process-table-validation-k37.

## Context
Read all ancestor briefs and docs/specs/diagnostic-process-inspection.md in full.
k37 tightens table acceptance in the actual synchronous controller only; it does not
bound helper creation, capture, waits, worker lifetime or scheduler turns.

## Done when
- Deliver the full remaining parent contract: private helper-owning worker, fork
  isolation/admission, separate bounded protocol channels, shared controller scheduler,
  fixed deadlines and authority, sticky failure, concurrent shutdown/teardown,
  nonblocking reap and distinct residuals, with the complete required control inventory.
- Preserve k33/k36 signal ordering and k37 strict table acceptance, moving the parser
  boundary as appropriate. Reconcile the existing untrusted draft rather than trusting it.
- Extend Taskfile/input declarations, freeze new evidence with independent exact
  resource audits, and update the report without rewriting old runs or claiming guests.
- As the final producer act insert review-impl bounded-inspection before
  retained-evidence-k21 with **Reviews:** bounded-inspection-k30 and the complete
  parent runtime ownership/waits/destructors/concurrency/control-inventory audit.
  Do not schedule an empty integration. Decompose further if a focused session requires it.
