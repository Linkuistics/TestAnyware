# complete-worker-scheduler-k96

## Goal

Bind each persistent worker command and response to the controller run, exact worker session/PID, unique request, workload leader PID/PGID, signal epoch, phase and immutable query deadline. Reject a mismatched or malformed envelope before accepting a table or retiring a helper obligation.

## Context

Read k94 and its scheduled k97 review, the k28 brief chain and docs/specs/diagnostic-process-inspection.md. k97 found no actionable lifecycle defect. Runtime is experiments/exec-reliability/{controller,worker_capture,inspection}.py. Persistent startup/reuse is delivered; request tokens alone do not establish complete observation identity.

The complete original aggregate charter and every unfinished requirement now live byte-for-byte in complete-worker-scheduler-k98. Shared earliest-deadline scheduling, shutdown/teardown overlap, epoch closure at cutoff, all remaining controls and final Reviews: bounded-inspection-k28 review stay there. k28/k17 remain open.

## Done when

- Persistent commands and readiness/intent/creation/fork-failure/completion/result messages carry a strictly validated envelope with run, worker session/PID, request, workload PID/PGID, signal epoch, phase and deadline. Controller supplies actual run/workload/signal-attempt context.
- Matching responses preserve sequential reuse; wrong/missing/type-invalid identities fail sticky without retry. Invalid terminal identity never retires a helper obligation. Known matching identities remain cleanup evidence after failure.
- Retain frozen RED/GREEN real-transport controls, exact-child audits and appropriate full tests/lint/format/strict typing, preserving all previous evidence byte-for-byte. No guest/installed-app/unrelated-process action.
- Cut an independent review-impl of this narrowed artifact before the aggregate continuation. The one-shot compatibility path is explicitly outside this slice.

## Decisions (running log)

- 2026-11-15: Flat sibling split keeps an independently verifiable production transport improvement here and the complete aggregate charter in k98, because adding identity envelopes does not establish shared safety scheduling. k97 was read and found no actionable lifecycle defect.
- 2026-11-15: Use an immutable request scope and strict envelope on the persistent path. Keep the existing standalone one-shot controls compatible. Request tokens remain unique per dispatch; signal epoch counts every attempted termination signal including denied attempts. Final observations carry the actual controller run and pinned workload identity.
- 2026-11-15: The retained report is `experiments/exec-reliability/evidence/request-identity-k96/report.md`. Final frozen controls pass all 82 cases; the full suite passes all 579 with lint, format and strict typing. The same 82 case identities pass in both runs. All measured subjects are unchanged before/after, and final frozen sources match live sources.
- 2026-11-15: Final no-op-envelope and zero-epoch mutations produce 71 and two failing controls respectively. Earlier RED and fixture-failure attempts remain retained and are explained without crediting API absence or fixture failures as behavioral proof. The prior-evidence audit checks 119,509 paths with zero changed or missing.
- 2026-11-15: Cut independent review-impl complete-worker-scheduler-k99 ahead of complete-worker-scheduler-k98. No aggregate parent closes; k98 retains the original full charter and k99 challenges only this landed identity slice.
