# admission-worker-state-k63

**Reviews:** complete-worker-scheduler-k60

## Goal

Independently review k60's admission and failure-state receipt validation before the persistent-worker/scheduler remainder in k62 relies on it.

## Context

Read the producer commit by its stable handle, k60, k48, docs/specs/diagnostic-process-inspection.md and the k60 report section in docs/experiments/exec-reliability-diagnostic-foundation.md. Re-derive the genuine producer states in inspection.py against worker_capture.py; the k61 review identified ECHILD with retained ownership in the earlier positive control.

## Done when

- Trace complete admission schema and reason priority, native-observation failure states, strict counts/booleans/errnos/signal values and meaningful errors against the actual producer. Check coherent admission refusals survive as known no-helper failures and contradictions fail conservatively.
- Check error/creation/output/close coupling, ECHILD and ownership equivalence, terminal state and residual remedy consistency without conflating receipt self-consistency with independently authenticated helper identity or reap.
- Reconcile the 50 synthetic publications and genuine two-thread admission-refusal control with retained body witnesses, real worker transport, exact rejection reasons, parent timing and shutdown. Check the EIO replacement and separate contradictory ECHILD negative control. Keep synthetic helper evidence distinct from actual helper observations.
- Audit immutable evidence/k60-inputs-01, k60-red-01, k60-fixed-01, k60-fixed-02, k60-audit-01 and k60-audit-02. Check exact 45 RED failures, identical 251-case inventories, final lint/format/strict types, both failed attempts, before/after hashes, 46,714 preserved prior files and external selector/scratch reconciliation. Account for audit limitations and avoid promoting later absence into proof about unknown creation.
- Preserve k62's entire persistent bootstrap/gated publication/independent ownership/shared scheduler/control/final-review charter and all aggregate ancestors. No guest, privilege, installed-app, stalled-storage or ten-clone acceptance claim; 0/10 remains unavailable.
- Record actionable findings with source coordinates and evidence. Cut an adjacent integrate-review-impl only for findings worth acting on, before k62. Create no empty integration.
