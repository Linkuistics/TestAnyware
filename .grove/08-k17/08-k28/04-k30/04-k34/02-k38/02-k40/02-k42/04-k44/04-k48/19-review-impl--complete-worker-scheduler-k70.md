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
