# Caller scheduling boundary, k140

`capture_helper(..., drive=driver)` and `Worker.close(drive=driver)` give the caller every query, retirement and close registration. The driver accepts a single `Turn` and must synchronously advance it to `None` or raise, optionally composing companions. This includes safety Turns; returning with a pending Turn violates the driver contract. The runtime does not probe completion by taking an extra Turn or transfer scheduling to a private fallback. Default calls still resolve the module driver at invocation time; default close remains a no-argument call for existing adapters. k145 adds exception-safe channel finalization; see [the integration addendum](../caller-cleanup-k145/report.md).

This slice does not connect concurrent owners in `controller.execute`, make startup asynchronous, or establish public teardown overlap. Those requirements and the entire original charter/history remain in complete-worker-scheduler-k143. k28/k17 remain open; guest acceptance is 0/10 unavailable. No guest or installed application was modified.

## Evidence

All commands use the root Taskfile through `mise exec -- task`. `diagnostic:caller-prepare` freezes all thirteen Python sources, manifest, lock and Taskfile. `diagnostic:caller-check` uses isolated Python 3.12 with controls/full/static budgets of 60/120/30 seconds, checker TERM 2 seconds then KILL/reap 2 seconds. Each result includes exact arguments, before/after subject hashes, XML item outcomes, stdout/stderr, fixture journals and artifact hashes. All ten recorded runs have identical before/after subjects; every checker was reaped without timeout.

| Condition | Result |
| --- | --- |
| RED against old runtime | 4 passed, 6 rejected: unsupported driver arguments |
| Parent full | 923 passed, 1 failed; Ruff, format and strict Pyright passed |
| First candidate controls | 7 passed, 4 failed: omitted Worker.close signature change |
| Candidate v2 controls | 10 passed, 1 failed: finalization fixture expected only the initial failure text |
| Candidate v3 controls | 11 passed |
| Candidate v4 full | 931 passed; 6 audit-tool tests, Ruff, format and strict Pyright passed |
| Query bypass mutant | 6 passed, 5 rejected |
| Retirement bypass mutant | 8 passed, 3 rejected |
| Close driver bypass mutant | 7 passed, 4 rejected |
| Capture close forwarding bypass mutant | 8 passed, 3 rejected |

The v2 correction preserves the initial sticky failure and requires the shutdown-overrun suffix produced at the intentionally expired clock. It changes no runtime allowance. v4 differs from v3 only in import ordering, corrected before the full/static checks. Failed samples remain intact; observed-deviations.json records the intermediate corrections. The final runner also retains the six missing-journal audit controls as a bounded checker after the suite.

The eight real transport cases cover default/supplied × standalone/persistent × success/stalled creation. A supplied driver makes any fallback to the module driver raise. Competing owners exercise safety priority; the existing controls enforce bounded reads, zero-timeout transport polling, readiness registration and terminal idempotence. Three deterministic controls cover direct close, exhausted allowance and finalization discovered while a caller exception propagates. The last uses an already-reaped synthetic owner and creates no child or signal.

Each mutant changes exactly one asserted source site. The retirement, close and capture-close mutants fail caller-escape assertions. The query-bypass mutant has three caller-escape rejections and two secondary default-driver rejections, `assert (1 == 2)` at frozen `test_turns.py:447` in `test_request_turns_yield_with_transport_priority_and_caps[True-False-False]` and `[True-True-False]`. These are direct test failures, separately from the incomplete-journal verdict. Missing completion journals remain explicitly unresolved. All observed exact fixture identities were independently absent at audit: RED 6; parent 31; initial/v2/v3 controls 8 each; final full 35; each mutant 8. No audit sent a cleanup signal. The successful candidate has all eight caller start/completion pairs and all four expired-request audit pairs. The original caller audit counted pairs without validating their contents; k145 records that limitation and adds content validation for new runs. Journal completion is accounting evidence, separately from process absence; it does not authorize signals.

## Comparison and limits

comparison.json records every item. There are 920 unchanged node IDs, four parent-only IDs replaced by the expanded parameterization, and eleven candidate-only IDs. Parent `test_late_created_identity_rejects_invalid_fields[future]` failed while the candidate passed. This is a retained baseline failure, with no causal repair claim. Earlier historical timing failures also remain binding; a clean suite here does not discharge them or establish scheduler integration.

The four mutants exercise driver routing, not every scheduler invariant. The tests demonstrate supported caller composition at a synchronous phase boundary, not fairness among production owners or failure retirement overlapping teardown. No independent adversarial review ran in this session; a separate review-impl leaf must assess this API and its evidence before controller integration relies on it. The final aggregate review remains Reviews: bounded-inspection-k28.

Graph discovery used project Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14, generation 2026-11-15T02:19:52Z. Relevant coverage was stale or not tracked; material runtime/tool claims use direct current source reads rather than absence claims from that index.

The original before/after preservation inventory contains 281,347 prior evidence paths with identical keys and hashes, including symlink targets. See prior-before.json and prior-after.json. At the k140 commit, runtime and tests matched candidate-v4 byte for byte. The k145 integration changes the current runtime/tests and this report; all retained k140 conditions remain unchanged. Its addendum also records the previously omitted runner hash change.
