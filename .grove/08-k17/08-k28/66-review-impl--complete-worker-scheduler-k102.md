# complete-worker-scheduler-k102

**Reviews:** complete-worker-scheduler-k98

## Goal

Independently challenge the worker-close escalation and ownership boundary delivered by k98. Report source-backed findings and retained verification evidence; do not implement fixes in this review.

## Context

Read the committed k98 producer, k28 brief chain, relevant lifecycle specification, and experiments/exec-reliability/evidence/worker-close-k98/report.md. The artifact is Worker.close and its owner/signal/reap callers in experiments/exec-reliability/worker_capture.py, seven close modes and ten cutoff controls in test_worker_capture.py, the Taskfile entry point and frozen worker-close-k98 evidence. Read RED as well as GREEN/full and distinguish runtime behavior from independent fixture cleanup.

The bounded contract gives a healthy worker one fixed orderly EOF grace, then TERM and KILL/reap within the original allowance. Failure or existing retirement receives no fresh healthy grace. Earlier TERM/KILL attempts, including denials, must not be duplicated or renew authority. An earlier finite owner deadline wins, repeated close is inert, and expired authority retains an explicit residual. Runtime success and an independent audit reap are different claims.

The original aggregate charter and history are preserved verbatim in complete-worker-scheduler-k101. Shared earliest-deadline scheduling, concurrency with controller teardown, epoch closure, first expired reap behavior and the remaining aggregate controls/final k28 review belong there. Synchronous close alone is not aggregate completion. k28/k17 remain open; ten-clone acceptance is unavailable 0/10 and storage k18 is separate.

## Questions to challenge

- Trace close admission and all preceding retirement paths. Can a previous denied TERM, old TERM timestamp, KILL, finite deadline or command-close failure cause skipped TERM, repeated attempts, renewed grace or lost owner authority? Check both ready group and PID-only targets where material.
- Challenge the 50ms grace and TERM/KILL/reap arithmetic against the original four-second allowance and earlier finite deadlines. Does close ever signal or probe after cutoff, claim successful reap without acknowledgment, or obscure a distinct helper/workload obligation? Separate pre-existing aggregate gaps from regressions in this slice.
- Assess the seven real-process close modes and ten cutoff controls. Can timing slack, injected observation behavior, fixture signals or manual ledger cleanup conceal a bounded defect? Are failed-attempt history and phase bounds adequately established? A real EPERM fixture was not executed; do not treat that as proven.
- Verify frozen subjects, command order/bounds, RED failures, GREEN/full records, independent exact-child audit and prior-evidence preservation. Determine whether retained observations support each report claim and expose any unsupported generalization.

## Done when

- Review committed source and relevant caller/callee relationships, disclosing graph coverage/freshness and source fallback limits. Material conclusions cite exact source and evidence.
- Retain a readable findings report and bounded verification artifacts without altering prior evidence. Explain historical failures from evidence rather than assuming a later pass repairs them.
- Follow review-impl findings/reconciliation routing. Cut integrate-review-impl before k101 only if actionable findings earn it. Leave k101 and parent aggregate open; do not duplicate the future complete-runtime review of k28.
