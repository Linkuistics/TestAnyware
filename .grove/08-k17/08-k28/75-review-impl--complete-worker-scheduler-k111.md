# complete-worker-scheduler-k111

**Reviews:** complete-worker-scheduler-k107


## Goal

Independently review the first worker exact-child nonblocking reap at an expired
cutoff. Establish whether this bounded ownership repair preserves all later
cutoffs, signal restrictions, residual accounting and sticky timing failures.



## Context

Read the k28 brief chain, docs/specs/diagnostic-process-inspection.md, narrowed
k107 producer and experiments/exec-reliability/evidence/worker-first-reap-k107/report.md.
Runtime is experiments/exec-reliability/worker_capture.py: _Owner, _reap, _signal,
Worker.close, capture_helper and their callers. Controls are test_shutdown.py
and test_worker_capture.py; inspect Taskfile and all four evidence tooling files.
The complete original aggregate charter/history is preserved in k110. This
review precedes k110, so cited source will not drift before review/integration.

## Done when

- Trace actual owner construction, every reap/signal entry point, close/finalizer
  reentry and dataclass evidence consumers. Verify that the first exception is
  consumed before waitpid on no-result, errno and late return, and cannot grant
  later polls, signals or renewed phase deadlines. Check terminal and residual
  states separately; a late successful reap must retain failed timing.
- Read RED and GREEN/full, all three one-line mutants, exact-case JUnit and raw
  fixture receipts. Verify frozen subjects and shipped-source equality,
  source-comparison.json, condition-audit.json and full prior inventories.
  Distinguish actual child status consumption from synthetic time/errno and the
  independent cleanup. Inspect audit-initial.py.txt and the annotation-only final
  audit.py, including the disclosed initial tooling diagnostics.
- Challenge before/equality/after cutoff, pre-ready/ready owners, live/exited
  child, late successful/no-result/error return, ECHILD/EIO, prior attempts and
  stale close/finalizer callbacks. Check that the 408 audits do not overclaim
  unrelated helpers, and that 30 late full-suite grace outcomes remain failures.
- Verify that k110 retains every original requirement and the final k28 review.
  Do not treat this slice's 754-case pass as shared-scheduler, overlap, privileged
  fixture or 0/10 guest acceptance, or as repair of historical flaky samples.
- Retain a findings report without changing earlier evidence. Route actionable
  findings to one integrate-review-impl sibling before k110; cut no empty
  integration when there are none. Leave aggregate ancestors open.

## Notes

The producer's environment had no GROVE_PROVIDER_EVIDENCE. Creator is deliberately
omitted under retire guidance; if review policy requires one, the owner must
declare the provider. Never synthesize evidence from another task or legacy run.
No guests, privilege acquisition, installed app/global settings or old evidence
edits. Routine bounded verification must freeze inputs/tools/bounds first,
retain all failed samples and separate runtime outcomes from fixture cleanup.
