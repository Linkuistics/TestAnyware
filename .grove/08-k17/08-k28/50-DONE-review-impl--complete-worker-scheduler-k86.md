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

## Decisions (running log)

- 2026-11-15: Review of `complete-worker-scheduler-k83` found the runtime change correct and three evidence gaps. Those gaps earn one integration leaf, `worker-deadline-evidence-k87`, placed ahead of k85.
  - Source, verified without finding: `worker_capture.py:44-61` (`_reap`) and `:64-79` (`_signal`) are the only worker signal and reap entry points. The deadline is set once at owner creation (`:488`). Both guards use `>=`, so equality is closed. The call sites at `:498`, `:613-627` and `:651` are the whole set. `controller.py` and `inspection.py` signal and reap only their own children.
  - Source, verified without finding: readiness scope (`:73-76`), `WNOHANG` and residual ownership in `_owners` are unchanged. The late-creation cleanup code at `:406-464` is untouched.
  - Evidence, verified without finding: `red-inputs` and `red2-inputs` match the parent `worker_capture.py` by hash. `green-inputs` matches the committed file. The RED 1 and RED 2 JUnit identities match. Each RED case shows at least one late OS call, and no GREEN case shows one.
  - Finding 1: the full self-check passed in 54.29 s inside a 60 s budget (`self_check.py:46`, `full-1/declaration.json`). k81 took 44.54 s. Eight new cases of 0.3 s explain only about 2.4 s of the 10 s growth. The report does not state this 5.7 s margin. k85 adds more cases, so a timeout is likely to appear as a false failure.
  - Finding 2: `audit.py` compares `green-inputs` with the live sources, and the report tells the reader to repeat the audit on a copy. After k85 edits `worker_capture.py`, that repeat fails by design. The report also cites a "separate jj path comparison" that no retained artifact holds. The audit verdict and its absolute worktree paths are valid only for commit c6844b2.
  - Finding 3: the stale-TERM, stale-KILL and at-cutoff cases also call `_reap` after the signal (`test_worker_capture.py:76-82`). A runtime that guards only `_signal` fails the reap check in those cases as well. RED ran only against the fully unguarded parent. No mutation shows each guard failing alone. The at-cutoff case sets the clock only for stale calls after the runtime returned. It does not exercise equality inside the cleanup loop.
  - Limit, accepted: a clock sample cannot bound preemption or a slow OS call. The report states this accurately. The adapter simulates delayed observation, not kernel unreapability. The report says so.
  - Limit, accepted: the prior-evidence inventory excludes symlink text and directory links. The audit verdict and report both state it.
  - Split: k85 keeps the whole aggregate charter. Ten-clone acceptance stays 0/10 unavailable.
