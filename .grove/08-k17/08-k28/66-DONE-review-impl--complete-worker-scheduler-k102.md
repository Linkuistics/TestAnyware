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

## Decisions (running log)

- Review outcome: no runtime defect in the bounded k98 contract. Three low-severity test-strength findings: F1 (no denied-signal history or PID-only target in the seven modes), F2 (the `expired` mode cannot detect a post-cutoff reap probe) and F3 (no command-close failure test). The report is `experiments/exec-reliability/evidence/worker-close-k102/report.md`.
- The review ran no test, build, lint or format command and executed no mutation. It ran read-only hash and byte-comparison commands only. The report states which claims rest on source reading alone.
- Coverage: the code graph predates commit `2228059`. The review used direct source reads and git for every cited path and disclosed that fallback in the report.
- Evidence verification: GREEN and full inputs equal `HEAD`. RED ran the new tests on the parent runtime. All retained artifact hashes match `HEAD`. The 51 exact-child audits are distinct and reaped. Prior evidence is unchanged, because every changed path under `evidence/` is in `worker-close-k98`.
- Observations kept out of the findings: wide timing slack, `retiring` treats any finite deadline as retirement, the in-process audit, the lost RED/GREEN runner bytes and the four preservation modes that passed on the unchanged runtime. The report lists each with its source.
- Pre-existing and aggregate gaps stay with complete-worker-scheduler-k101: the plan declaration of the orderly-close grace, shared scheduling and overlap with controller teardown. The review did not duplicate the final k28 review.
- Routing: all three findings are actionable. The review cut `complete-worker-scheduler-k103` (integrate-review-impl) before k101. It left k101 and the parent aggregate open.
- No `**Creator:**` line applies, because the environment carries no provider evidence.
