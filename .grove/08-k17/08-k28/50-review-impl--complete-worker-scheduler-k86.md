# complete-worker-scheduler-k86

**Reviews:** complete-worker-scheduler-k83

## Goal

Independently challenge the worker shutdown-deadline boundary delivered by k83 before complete-worker-scheduler-k85 consumes it. Derive findings from the committed artifact and its contract; this review does not implement the persistent-worker/shared-scheduler aggregate.

## Context

Read k83's committed diff, experiments/exec-reliability/worker_capture.py, its regression controls in test_worker_capture.py and experiments/exec-reliability/evidence/worker-deadline-k83/report.md with the retained raw conditions and audit. Read docs/specs/diagnostic-process-inspection.md and bounded-inspection-k28 for the ownership/deadline contract. The entire aggregate remains assigned to k85; k28/k17 are open.

## Done when

- Trace every worker signal/reap entry and call site, owner creation, readiness transition and residual lifetime. Challenge whether the original cutoff closes both entry points, including equality and stale callbacks, without changing helper-local ownership or cleanup semantics.
- Examine the timing limits between clock samples and OS operations, reaping versus identity pinning, and failure/overrun receipts. Check whether the contract and report state the practical limits accurately.
- Challenge the real-child controls and their deferred-wait adapter, exact-child fixture audits, frozen RED/GREEN case identities, full-suite coverage, executable hashes and prior-evidence inventory limitations. Distinguish adapter behavior from OS unreapability and privileged evidence.
- Report actionable findings with exact source/evidence references and their contract basis. If integration is needed, insert it ahead of the first later live sibling in this directory; otherwise create no integration. This slice review does not replace the eventual aggregate bounded-inspection-k28 runtime review.
