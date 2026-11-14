# isolated-helper-capture-k53

**Reviews:** isolated-helper-capture-k51

## Goal

Review the committed, live per-query worker isolation slice before further runtime changes. Validate k51's bounded useful behavior and conservative ownership claims while keeping every complete-worker-scheduler-k52/ancestor requirement live.

## Scope and evidence

Read k51, unchanged k48 charter, complete-worker-scheduler-k52, the ancestor worker/scheduler briefs and docs/specs/diagnostic-process-inspection.md. Inspect experiments/exec-reliability/{worker_capture.py,inspection.py,controller.py,test_worker_capture.py,pyproject.toml,self_check.py}, the report in docs/experiments/exec-reliability-diagnostic-foundation.md and evidence/k51-* beneath experiments/exec-reliability.

The final runner k51-final-01 records all four checkers passing and 142 tests, including the channel-close regression. k51-check-01 retains an earlier strict-type failure; k51-check-02 passes 141 tests. k51-red-01 and k51-close-red-01 retain targeted failing JUnit without full runner envelopes. The initial Darwin pipe2 platform failure has no separate artifact. k51-audit-01 failed on a historical receipt-schema mismatch before process checks/cleanup; k51-audit-02 retains that failure, validates each historical schema and final input/artifact hashes, challenges its process absence predicate and accounts for 1256 exact recorded PIDs and 285 scoped fixture directories. The close-error RED lost its receipt and worker PID. Later absence never repairs an original unknown-creation decision.

## Review questions

- Trace all worker creation/read/close/reap/signal/exception/child-exit paths. Does blocked helper construction leave the controller within the original deadline, with bounded nonblocking I/O and conservative residuals? Is raw worker fork's remaining synchronous assumption honest? Do channel exceptions preserve all closure and final receipt obligations?
- Verify every worker fork admission, blocked-signal interval, parent mask restoration, native/Python SIGCHLD handling, child descriptor/handler/mask/wakeup reset, fresh owner maps and os._exit behavior. Inspect inherited hooks/destructors and any unexpected inherited signal authority. Every-helper reset remains undelivered; ensure no text or adapter suggests otherwise.
- Verify exact child ownership, PID-only shutdown before readiness, session evidence before group authority, group/PID shutdown order, ECHILD handling, no signal after reap or ownership loss, immutable outer deadline and conservative unknown helper creation. Worker/group disappearance must never stand in for helper reap acknowledgement.
- Challenge capped channel reads/frames/decoded output, malformed receipts/timestamps/token/identity handling and close/shutdown races. Identify missing proof separately from a demonstrated defect. Full run/request/workload/epoch validation, gated helper publication, independent known-helper obligations and one safety-first scheduler are k52 work, not delivered k51 claims.
- Verify live controller use and frozen/typecheck subjects. Distinguish real worker controls from direct helper engine and injected return-value/errno adapters. Check independent audit identities, fixture whitelist, all original failed artifacts and limits. No VM/app/privilege action or changed 0/10 unavailable acceptance is authorized by this review.

## Done when

Record findings against committed file/line evidence, separating k51 regressions or unmet slice criteria from the complete k52 remainder. Add an integrate-review-impl step only for actual findings, inserted before the next live sibling entry so cited source does not drift. Do not close k48 or any ancestor. The final whole-runtime review of bounded-inspection-k30 before k21 remains a k52 producer obligation.

## Provenance

The finishing session had no GROVE_PROVIDER_EVIDENCE in its own environment. Grove requires no Creator line in this case; if caller policy refuses launch, the owner must declare the provider. Do not invent or borrow a receipt.
