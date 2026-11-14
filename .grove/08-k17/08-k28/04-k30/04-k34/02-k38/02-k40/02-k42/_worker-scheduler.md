# worker-scheduler-k42 — brief

## Goal
Complete every remaining worker-scheduler-k40/k38 requirement after k41.

## Context
Read all ancestor briefs and docs/specs/diagnostic-process-inspection.md in full.
k41 delivers only the single-final-observation rule in the actual synchronous
controller. It keeps fixed deadlines/reap ownership from k39. inspection.py is
still an untrusted draft; synchronous helper creation/capture/waits may overrun.
The complete k40 Done when remains authoritative and must not be narrowed.

## Done when
- Deliver the private helper-owning worker, native/Python thread and SIGCHLD
  admission before every fork, child signal/wakeup-fd/descriptor reset, gated
  publication and bounded separate channels, with all specified fault adapters.
- Integrate the actual shared safety-first scheduler, immutable deadlines and
  closed authority, sticky worker failure, concurrent resource teardown and
  worker shutdown, nonblocking helper/worker reaps and strong residual owners.
  Preserve k33/k36 ordering, k37 parsing, k39 ownership and k41 single query.
- Run the complete specified real control inventory, including all delayed fork,
  publication/exec/reset/protocol/cutoff/teardown/shutdown cases. Keep simulations,
  real helper KILL timing and unavailable genuine UID denial distinct.
- Freeze inputs/receipts, independently audit every known fixture/helper/worker,
  update the durable report and preserve historical evidence byte-for-byte.
- As the final producer act insert review-impl bounded-inspection before
  retained-evidence-k21, with **Reviews:** bounded-inspection-k30. Require actual
  runtime ownership, blocking waits/destructors, authority boundaries, concurrency
  and the complete control inventory to be audited. No empty integration.
  Decompose only at independently useful green behavior boundaries; carry every
  remaining parent requirement into live work.

## Notes

## Decisions (running log)

- Split out k43 because an invalid table can currently be followed by fresh
  teardown queries that mark disposal certain. Run-scoped sticky failure and
  continuing public safety commands are independently useful and testable in
  the current controller. k44 carries the complete remaining worker/scheduler
  contract and final bounded-inspection review; no requirement is discharged by
  decomposition. k43 has its own specific material review before k44.
- Use an explicit execution-owned inspection state passed through private call
  paths. Latch unknown observations from attempted queries, preserving the first
  call and raw receipt;
  valid live observations remain termination failures rather than inspector
  failures. No global state or replacement query is introduced.
- k46 integrates k45: `GroupObservation.query_attempted` distinguishes skipped
  final queries (missing leader exit or exhausted cutoff) from a failed helper.
  A skipped query fails its call and stops remaining workload without disabling
  later independent safety inspection. A real `/bin/ps` usage-error control
  confirms query failures still disable every later local table and probe.
  k46-final-01 passes 119 tests, Ruff and strict Pyright; k46-audit-01 accounts
  for 300 real PIDs, 72 markers and 92 argv-derived scratch directories.
  The old k43 valid-live replay failed on a missing receipt field, not changed
  query continuation; the durable report corrects that interpretation.
  k44 retains the complete remaining contract and final subsystem review.
