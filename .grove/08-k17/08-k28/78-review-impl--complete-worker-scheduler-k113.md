# complete-worker-scheduler-k113

**Reviews:** complete-worker-scheduler-k110

## Goal

Independently challenge incremental worker shutdown against the producer's
narrowed contract and the applicable shutdown/ownership spec.

## Context

Read the k110 producer, its evidence report and the implementation in
experiments/exec-reliability/worker_capture.py plus test_shutdown.py.
The full aggregate remains complete-worker-scheduler-k114; do not certify it
from this refactor. Read worker-reap-scope-k112 and shutdown-phases-k109 for
owner-lifetime first-reap and strict phase timing limits.

## Done when

Trace the real begin/advance/close consumer behavior and challenge nonblocking
progress, one fixed timeline, due signals, prior denied attempts, expired
owner history, idempotence and retained phase evidence. Check real controls
and independent cleanup audit, and attempt a named regression against the
bounded contract. Report findings with evidence, limits and disposition
questions, without fixing them. Reconciliation is earned only by findings.

## Notes

No guests/privilege/installed changes or unrelated processes. Keep every prior
sample. Shared earliest-deadline scheduling, actual cleanup overlap, all
remaining aggregate controls and the final k28 review remain k114's work.
