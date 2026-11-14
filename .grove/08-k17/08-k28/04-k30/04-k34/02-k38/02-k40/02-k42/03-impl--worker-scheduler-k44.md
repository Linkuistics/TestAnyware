# worker-scheduler-k44


## Goal
Complete every remaining k42 and ancestor worker/scheduler requirement after
the reviewed sticky inspection failure slice.

## Context
Read the entire ancestor chain and docs/specs/diagnostic-process-inspection.md.
k43 changes only run-scoped sticky failure and continuing safety commands.
inspection.py remains an untrusted worker draft. Synchronous helper creation,
capture and post-timeout waits remain; no worker/scheduler guarantee is delivered.

## Done when
- Satisfy the complete k42 Done when and the full k40/k38/k34/k30/k28 contracts:
  private helper-owning worker, every-fork admission/reset, gated publication,
  separate bounded channels and strict identity/time/framing validation.
- Integrate the one safety-first scheduler with immutable workload/teardown/
  worker clocks, exclusive controller signal authority, bounded nonblocking
  reaps, sticky worker retirement and strong distinct residual owners.
- Preserve k33/k36 ordering, k37 parsing, k39 ownership, k41 sole final query
  and k43 no retry after inspection failure, including continuing safety commands.
- Run every required real control, including delayed creation/publication/exec,
  inherited-state reset, protocol/cutoff/backpressure, concurrent teardown and
  worker shutdown. Keep simulated unreapability, real helper KILL timing and
  unavailable genuine UID denial/setuid-ps controls separately identified.
- Freeze inputs and bounds, retain all samples, independently audit every known
  fixture/helper/worker, extend runner/type-check subjects and update the report.
- As the final producer act insert review-impl bounded-inspection before
  retained-evidence-k21, with **Reviews:** bounded-inspection-k30, requiring the
  actual-runtime ownership/wait/destructor/authority/concurrency/control audit.
  No empty integration. Decompose only useful green behavior boundaries while
  carrying all remaining parent requirements into live work.

## Notes
k21 immutable evidence and k18 storage isolation remain separate live obligations.
