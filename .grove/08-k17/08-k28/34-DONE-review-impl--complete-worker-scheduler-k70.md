# complete-worker-scheduler-k70

**Reviews:** complete-worker-scheduler-k67

## Goal

Independently challenge helper publication before exec and controller retention of
published helper obligations in the committed k67 artifact. Report findings rather
than implementing fixes. Read the implementation and contract before relying on
the producer's report or expected test outcomes.

## Context and artifact

The bounded producer is complete-worker-scheduler-k67. Its runtime files are
experiments/exec-reliability/inspection.py and
experiments/exec-reliability/worker_capture.py; controls are in test_inspection.py
and test_worker_capture.py in that directory. Evidence is in
experiments/exec-reliability/evidence/k67-{red-01,green-01,green-02,green-03},
k67-inputs-01, k67-audit-{01,02}, and helper-publication-k67/report.md.

Read docs/specs/diagnostic-process-inspection.md and worker-scheduler-k48 for the
inherited contract. This review owns the narrowed k67 slice. The full scheduler
charter, complete fault inventory and final aggregate review remain live in
complete-worker-scheduler-k69. Do not use local passing checks as aggregate
acceptance, or widen this review into implementing that remainder.

## Contract and specific doubt

A helper must reset inherited state, then wait on a private token gate with a
fixed deadline. EOF, a wrong token, expiry or failed creation publication must
prevent exec. The worker must finish publishing PID, PPID, PGID, per-query token
and creation time before gate release, without a controller acknowledgement.

The controller must validate worker/dispatch identity, schema and timing, reject
duplicate or conflicting publications, and match a terminal helper receipt to the
published identity. A known helper is a separate obligation retained through
worker/result loss and ECHILD. Only a matching reaped helper receipt resolves it;
worker reap or group signals never do. Publication grants no helper signal
capability. Channel consumption must remain capped, including consumed records.

Try to break the boundary at partial publication, worker death before/after gate
release, deadline expiry and terminal receipt acceptance. Check whether identity
validation or cleanup can erase a known helper, turn an uncertain creation into
absence, grant stale-PID signal authority, or permit exec before complete
publication. Distinguish actual kernel controls from injected message corruption
and synthetic terminal fixtures. Challenge whether the retained evidence and
resource auditor support their stated scope, especially unknown unpublished
helpers and later PID absence versus matching reap acknowledgement.

## Done when

Record actionable correctness/contract/evidence findings with exact source and
reproduction boundaries, or explain the reviewed scope and residual limitations
when none are found. Follow Grove review/integration routing for any findings;
place necessary integration immediately before complete-worker-scheduler-k69.
Leave all aggregate ancestors open. No guest/app edits, privilege acquisition,
stalled-storage or ten-clone acceptance claim is authorized by this review.

## Result

**Findings: none that need integration.** This review read the committed k67
artifact at f4a513fb8725. It ran no tests and changed no code. The
integration leaf is not cut.

### Reviewed and found sound

1. **Gate before exec.** `_await_gate` (inspection.py:171) fails on EOF, a wrong
   token, a late token and a short read. `_helper_child` (inspection.py:206) exits
   71 on failure, and exec follows only a true result. The child closes the write
   end through `reset_after_fork`, so parent death produces EOF.
2. **Publication before release.** `_spawn_helper` (inspection.py:229) calls
   `publish_created` after the fork and before the token write. A raised
   publication, an expired deadline and a short write all leave the gate closed.
   The `finally` block then kills the exact unreaped child.
3. **Controller identity checks.** `_created` (worker_capture.py:181) enforces the
   exact key set, the per-query token, `ppid == pgid == worker pid`, the deadline,
   the dispatch window and exclusion of `bool`. A duplicate line keeps the first
   identity (worker_capture.py:428).
4. **Retention.** `_helper_obligations` is written at worker_capture.py:431 and
   removed only at worker_capture.py:465, after pid and `created_monotonic` match a
   reaped result. Worker reap, worker loss, ECHILD and decode failure all keep the
   entry. A rejected publication reports `unknown_helper_creation`, not absence.
5. **Signal authority.** `_signal` (worker_capture.py:60) targets only the worker.
   It is guarded by `reaped` and `reap_errno`. Publication adds no helper signal
   path. The residual remedy says that group signals do not prove helper death.
6. **Channel caps.** `control_bytes` counts every consumed control byte against
   4096, so consumed records stay capped.
7. **Evidence.** The k67-green-03 subject hashes match every committed source, test
   and config file. The audit controls separate present, absent, invalid and
   present PIDs. The auditor states its limits: later PID absence is not a reap
   acknowledgement, and unpublished helpers stay unknown.

### Residual limits (no action for k70)

- **Test power.** The `expired` gate test cannot tell which of two independent
  checks refused release: the parent deadline check (inspection.py:277) or the
  helper deadline check (inspection.py:185). Both give exit 71 and no marker.
  Removing either one alone would keep the test green. This is deliberate
  redundancy. Pin it only if k69 wants a mutation-style control.
- **Injected faults.** The `eof` and `wrong` gate tests monkeypatch `os.write`.
  The `created-conflict` tests corrupt worker messages in process. Neither is a
  kernel fault, so neither proves behaviour under a real short write.
- **Synthetic fixtures.** Terminal fixtures use pid 987654321 and publish through
  `publish_created`. They check controller validation, not a real helper.
- **Claimed pgid.** The published `pgid` is the worker's `os.getpgrp()` at fork. It
  is not read back from the kernel for the helper. The helper keeps the worker's
  group (`new_session=False`), so the claim holds today.
- **Conservative over-reporting.** A worker that dies before `intent` still
  reports `unknown_helper_creation`. This errs toward "unknown", never "absent".
- **Ledger lifetime.** `_helper_obligations` is in-process and has no consumer
  outside tests. The controller surfaces only `residual_remedy` text
  (controller.py:243). Durable reconciliation across restarts belongs to the
  remaining k69 charter.
- **Evidence scope.** PID absence is checked at one later time on a PID space that
  can wrap. The audit cannot show that no unpublished helper exists.

No guest, privilege, stalled-storage or ten-clone claim follows from this review.
k69 keeps the full scheduler charter and the final aggregate review.
