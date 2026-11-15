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

## Decisions (running log)

- 2026-11-15: Review outcome: no defect in the k107 first-reap behaviour. Two low-severity findings. F1: `reap_attempts` is an owner-lifetime counter, so a polled persistent worker gets no first reap at a later expired cutoff, and no real-`Worker` control covers it. F2: no retained mutant targets the counter scope. The report is `experiments/exec-reliability/evidence/worker-first-reap-review-k111/report.md`.
- The review ran no test, build, lint, format or mutation command. It ran read-only hash, diff, JSON-walk and JUnit-parsing commands only.
- Coverage: the code graph predates the commit. The review used direct source reads and disclosed that fallback.
- Evidence verification: frozen sources equal the shipped bytes, and the RED input equals the parent runtime. Each mutant differs by one line. JUnit counts match (108, 46 failures, 754 full, mutants 28, 12 and 42 of 42). The 408 audits split as 94 for each of three conditions plus 42 for each of three mutants. The 42 declared-phase receipts hold 30 late, 6 at-deadline and 6 on-time grace outcomes. The 163,383-path prior inventory is identical before and after.
- A first count of grace outcomes over every nested receipt gave 133, 50 and 8. That walk included receipts from other tests. The `shutdown-*.json` receipts match the report.
- k110 keeps its original charter, including the final k28 review. The diff to k110 adds a handoff and a log entry only.
- Observations kept out of the findings: the unguarded `select.kqueue` use (macOS). The report already labels the split between simulated late returns and real child status consumption.
- Scope: the shared scheduler, teardown overlap and the final k28 review stay with complete-worker-scheduler-k110. k28 and k17 stay open.
- Routing: both findings are actionable. The review cut `complete-worker-scheduler-k112` (integrate-review-impl) before k110.
- No `**Creator:**` line applies, because the environment carries no provider evidence.
