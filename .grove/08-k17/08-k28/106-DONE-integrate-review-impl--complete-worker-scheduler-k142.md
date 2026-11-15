# complete-worker-scheduler-k142

**Integrates:** complete-worker-scheduler-k141

## Goal

Triage the findings of complete-worker-scheduler-k141 against the k137 expired-request control repair. Apply the real part narrowly. Leaf k140 keeps concurrent production owners, teardown overlap and every historical failure.

## Context

Read the k141 Decisions log (findings F1 to F3), then `experiments/exec-reliability/evidence/expired-request-controls-k137/report.md`. The control is `test_persistent_worker_expired_request_is_sticky` in `experiments/exec-reliability/test_worker_capture.py`. The runtime sites are `Worker.evidence` and `capture_helper` in `experiments/exec-reliability/worker_capture.py`. The runner is `run_condition.py` in the k137 evidence directory.

## Done when

- Classify each of F1 to F3 (contract unclear, real issue, visible trade-off or noise) with evidence. Re-read the source and the retained receipts. Do not inherit the k141 reading.
- For F1, decide whether the project Python 3.12 gate needs a control that rejects the `false-residual` mutant. Any new control must use real child state. Do not add a skip-free path that consumes status through `os.waitid` on 3.12.
- For F2, decide whether a deterministic schedule can reach the collected branch of `natural` without a race. If none exists, record the limit in the report path that k140 reads.
- For F3, add a frozen mutant for the receipt remedy at `capture_helper`, and decide on ECHILD and `ownership_lost`. Decide how a missing journal is reported by the audit.
- Keep every prior sample, mutant and receipt unchanged. Use fresh output directories. Retain every failure.

## Notes

No guests, privilege, installed changes or unrelated processes. Do not accept this slice as concurrent ownership, teardown overlap or aggregate acceptance. k28 and k17 stay open. The historical failures listed in the k137 report stay owned by k140.

## Decisions (running log)

- 2026-11-15 triage: F1 is a real sensitivity gap: on project Python 3.12 the only collected control skips and false-residual is not rejected. Preserve a before sample, then prepare real exited state using bounded non-consuming `/bin/ps` zombie observation before the real exact-child runtime wait. No runtime source or interpreter pin changes.
- F2 is a visible trade-off: an unprepared natural exit races the first WNOHANG by definition. Deterministically delaying that check would be a different prepared schedule. Keep natural conditional on the actual wait result and publish its unobserved collected branch where k140 reads; deterministic collected sensitivity belongs to exited.
- F3(a) is a real sensitivity gap: add separate missing/false receipt-remedy mutations; add a real external-reap/ECHILD schedule and mutations for ownership-loss bookkeeping. ECHILD means runtime did not collect status: keep reconciliation evidence, drop signal authority, and never equate it to successful runtime reap.
- F3(b) is wording/contract ambiguity: exact_child_absent records an ECHILD assertion, not global process absence. Record the actual exception and timestamp and label its scope. F3(c) is a real audit gap: early fixture declarations plus explicit expected schedules must make missing completion journals unresolved, including zero journals. External PID-only presence stays conservative uncertainty; no signal or identity/reuse conclusion from it.
- Verification uses the Python/test-first and doubt-driven skills. Executable frozen mutation rejections and audit fault controls are the better instrument for these behavioral claims; no extra in-session adversarial reviewer. k140 and the final aggregate review retain their charters. Graph generation 2026-11-15T02:19:52Z located symbols, but coverage reports metadata_changed for live sources and not_tracked for k137 tools; all four material source files were read directly. No graph completeness claim.
- Evidence lives in `experiments/exec-reliability/evidence/expired-request-review-k142/report.md`, now a Read-first handoff on k140. The retained original false-residual 3.12 sample passes two/skips one, confirming F1; the repaired controls reject all ten named frozen mutants on 3.12 and six supplemental receipt/ownership mutants on isolated 3.15. Each mutant run independently resolves all four expired fixtures, preserves subjects and rejects at the intended behavioral assertion. No runtime source changed.
- The missing/start-only/invalid-completion audit controls were RED against the original audit and GREEN after explicit expected-schedule accounting. Actual ECHILD records replace the constant absence claim. PID-only external observations remain conservative; no new signal authority is granted.
- Serial full suites: parent 922 passed/1 skipped; candidate 924 passed; all 920 common JUnit cases passed in both. Candidate full Ruff/format passed but explicit Pyright checking of the audit test produced seven local-import/type errors; retain its false overall verdict. Remove only the test from explicit Pyright inputs per repository implementation scope, retain it in Ruff/format/hashes, and use the separate fresh passing static receipt. No behavior changed for that repair. Initial tool-development failures and original receipts remain visible.
- Final receipt verification confirms artifact digests, per-run subject stability, exact mutant failure sets, resolved fixture journals, unchanged runtime/config snapshots and 276,908 unchanged prior evidence paths. F2 remains a published limit: all three nonmutant natural journals record `[0, 0]`; deterministic preparation belongs to exited, not a renamed natural schedule. No 3.15 full-suite, concurrency, teardown-overlap or aggregate acceptance claim follows. k140 retains every historical failure and final review; k28/k17 stay open.
