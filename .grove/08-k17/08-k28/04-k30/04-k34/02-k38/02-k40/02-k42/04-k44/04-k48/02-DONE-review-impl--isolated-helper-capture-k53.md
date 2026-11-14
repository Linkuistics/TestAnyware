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

## Decisions (running log)

Review of producer k51, commit 0189003. Inspection only: no test, lint, build or format command ran. Lines are in `experiments/exec-reliability/` at that commit.

### Findings

- Finding 1 (medium, ownership): the child reset can clobber a channel descriptor when the controller starts with stdio closed.
  `inspection.py:309-313` closes every descriptor except the two channel ends. It then opens `/dev/null` for 0, 1 and 2 and takes the lowest free number each time.
  If a channel end holds 0, 1 or 2 (a parent started with a closed standard stream), the lowest free number can be an allowed end. `os.dup2(null, fd)` then replaces it. A simulation with write ends at 1 and 3 clobbered end 1.
  The worker then cannot publish. The parent reports unknown creation and the query is lost. The failure is safe but silent, and no test starts the worker with closed stdio.
- Finding 2 (medium, bound): a hostile or corrupt result frame can raise an exception that the parent does not catch.
  `worker_capture.py:117` calls `json.loads` on child-written bytes. Nesting of about 200000 levels raises `RecursionError`, which is neither `OSError` nor `ValueError`. A 1 MiB frame can hold that depth.
  `worker_capture.py:262` catches only `(OSError, ValueError)`. The `finally` block still reaps and records `receipt.worker`. The exception then leaves `capture_helper` and passes `_group_snapshot` (`controller.py:311`) and `_safe_call` (`controller.py:571`), which also catch only `OSError` or `ValueError`.
  One bad frame can therefore abort the safety continuation of a whole run. The same applies to `TypeError` or `AttributeError` from a malformed field that `_decode` does not reject. The worker is project code, so this is a robustness gap, not a demonstrated live defect. The readiness line is safe: its 4096-byte cap keeps nesting below the recursion limit.
- Finding 3 (low, missing proof): no test covers the capped or malformed worker channel.
  `test_worker_capture.py` has seven tests. None sends an oversized frame, duplicate keys, a bad token, a bad timestamp, a hostile receipt, a child exit before intent, or a partial frame. The cap and decode code at `worker_capture.py:108-167` and `:227-259` has no control.
- Finding 4 (low, test boundary): controller controls do not run through the worker.
  `test_controller.py:18` imports `capture_helper` from `inspection`. The controller tests wrap that engine with adapters through `monkeypatch.setattr(controller, "capture_helper", ...)`. They test controller reaction only. No control runs `_group_snapshot` through `worker_capture.capture_helper`.
  The report states the earlier scope honestly. The adapter results must not count as live worker evidence.
- Finding 5 (low, stale text): two comments still describe the pre-k51 state.
  `inspection.py:4` says "Synchronous creation and controller-local scheduling remain interim limitations". `inspection.py:182` says "the private worker is still required". After k51 the live path uses the worker, and only the engine inside the worker is synchronous.
- Finding 6 (info, k52 remainder): the parent trusts the child's own `reaped` and `pid` fields.
  The parent does not corroborate them. This is a stated k52 obligation (gated publication and independent known-helper checks). It is not a k51 defect.

### Checked and sound

- Admission runs before fork (`worker_capture.py:180-185`). Signals are blocked around fork and restored in `finally` (lines 199-210). The child ends in `os._exit` on every path.
- Shutdown before readiness uses the PID only. Group authority needs session evidence (`_signal`, lines 57-70). No signal follows a reap, and `ECHILD` drops the owner and records lost ownership.
- The outer deadline stays fixed. The KILL time is `min(deadline, now + reserve/4)` (line 272). Channel closure is guarded per descriptor (lines 300-305), and the close-error RED is retained.
- Caps hold: control 4096 bytes, frame `FRAME_CAP + 4`, decoded output 256 KiB, reads of `TURN_CAP // 2` per descriptor.
- Delayed creation is real. The marker PID equals the worker PID and its parent is the controller. The k51-final-01 receipts show TERM only for delayed creation, TERM and KILL for startup delay, and no signal for the healthy case.
- Evidence matches the report: k51-final-01 passes all four checkers with 142 tests. k51-check-01 keeps its strict-type failure and k51-check-02 passes 141 tests. k51-audit-02 passes and accounts for 1256 PIDs, all absent.
- k52 keeps the persistent worker, gated publication, the strict scheduler and the final k30 review. No text claims every-helper reset is delivered.

### Outcome

Findings 1 and 2 warrant integration, with Finding 3 as the proof that closes both. Integrate-review-impl leaf `isolated-helper-capture-k54` was cut before complete-worker-scheduler-k52. Findings 4 and 5 go to the same leaf as optional fixes with a stated reason if rejected. Finding 6 stays with k52.
