# complete-worker-scheduler-k144

**Reviews:** complete-worker-scheduler-k140

## Goal

Independently challenge the caller scheduling boundary before production controller integration relies on it. The complete scheduler charter remains live in complete-worker-scheduler-k143; k28/k17 stay open.

## Read first

Find the producer commit by its stable handle. Read the retired k140 task and experiments/exec-reliability/evidence/caller-scheduling-k140/report.md, comparison.json, final-verification.json, frozen parent/final candidate sources, runner/preparer/preserver/audit controls, all ten results and before/after evidence inventories. This finishing session had no GROVE_PROVIDER_EVIDENCE, so no Creator line is available; apply the caller policy without inventing evidence or borrowing another task's receipt.

## Review case

- Re-derive the contract for a supplied synchronous single-Turn driver. Trace query, retirement, exhausted allowance, ordinary failure close and finalization close, including exceptions. Challenge fallback resolution, monkeypatch compatibility, callback priority/readiness, fixed cutoffs, sticky failures, reaped-owner handling and descriptor ownership. Worker.start and controller.execute integration remain outside the delivered claim.
- Check whether the eight real default/supplied × standalone/persistent × success/stall cases establish caller-owned composition rather than hiding waits or cleanup outside the driver. Challenge the deterministic expired/finalization controls, especially their synthetic already-reaped owner and propagated caller exception. Assess whether helper creation before the driver, startup, terminal idempotence or failure retirement leaves an untested assumption.
- Evaluate all four one-site routing mutants, including exact rejecting assertions and secondary failures during mutant cleanup; distinguish those from missing-journal verdicts. Trace exact fixture journals, external absence audits and the eight expected caller journal pairs. Journal accounting is not process absence or signal authority. Challenge vacuous, malformed or mismatched journal acceptance and fixture cleanup scope.
- Reconcile RED, both intermediate candidate failures, the parent late-created identity future failure, final full 931 passes, six audit-tool passes, static results, 920 common node IDs/four replaced IDs/eleven candidate IDs, per-run subject hashes and prior evidence preservation. A later pass does not repair historical failures. Review runner changes between samples and the removed temporary isolated interpreter in final-verification.json.
- Assess reproducibility and proportionality of Taskfile/evidence tooling. Preserve every retained evidence path byte-for-byte. Do not launch guests, acquire privilege, change installed applications or signal unrelated processes.

## Done when

Publish a bounded independent review with precise findings and evidence or a supported no-findings conclusion. Cut integration only for findings worth acting on, following Grove adjacency rules. Keep the entire aggregate and prior failures in k143, including production owner scheduling, overlapping teardown and the final review with Reviews: bounded-inspection-k28 before k21. Guest acceptance remains 0/10 unavailable.

## Decisions (running log)

Review of producer commit 529a680 (k140). Inspection only: no test, build or lint command ran. Evidence came from parsing retained verdict.json, pytest.xml, comparison.json and report.md.

Confirmed, no finding:
- Counts reconcile. comparison.json has 920 common, 4 parent_only and 11 candidate_only IDs, and 1 change.
- RED shows 6 `drive` TypeErrors. The mutant pass/reject counts in report.md match the pytest.xml failure counts.
- subjects_before equals subjects_after in every run.
- All four mutants are rejected.

F1 (real, medium). A raising caller driver skips channel cleanup.
- `capture_helper` runs the retirement Turn at `worker_capture.py:1389`. That site is inside the `finally` that starts at line 1327.
- If the supplied driver raises there, lines 1390 to 1465 never run. That skips `_reap(owner)`, the `receipt.worker` population, `close_worker()` and the `for fd in channels: os.close(fd)` loop at line 1445.
- Result: the worker channel descriptors leak, and the caller sees an exception with no receipt.
- `test_caller_driver_owns_finalization_failure` raises only for the non-safety query Turn. No test raises for the retirement Turn or the close Turn.
- The default `run_turns` has the same shape. The caller-owned driver widens the exposure, because the caller controls when it raises.

F2 (contract gap, low). A supplied driver that returns without running its Turn is accepted silently.
- Query: `capture_helper` raises TimeoutError("worker query deadline exhausted") when the query is incomplete.
- Retirement: a no-op driver is followed by one `_reap(owner)` attempt.
- Close: `begin_close` has already run, so `Worker.closed` is True but `advance_close` never ran.
- No test uses a returning no-op driver. The synchronous-driver contract is stated in prose only.

F3 (evidence precision, low). report.md overstates the query-bypass mutant.
- report.md line 28 says each mutant fails "with the corresponding caller-escape assertion".
- Two of the five query-bypass failures are `assert (1 == 2)` at `test_turns.py:447` in the default-driver (supplied=False) cases.
- The mutant is still rejected, so the claim is wrong about the mechanism, not the outcome.

F4 (evidence integrity, low). The runner changed between samples and nothing records it.
- `run_condition.py` hash is ebb966a5 for RED, parent, candidate and candidate-v2.
- It is 649f85d1 for v3, v4 and all four mutant runs.
- Neither observed-deviations.json nor report.md lists the change.
- `caller_audits` in `run_condition.py` accepts a journal when it has one start and one completion with the declared name. It does not validate the journal contents, so a malformed journal passes. report.md already says "journal accounting only".

Placement: the next sibling `110-impl--complete-worker-scheduler-k143` was live, so the integration leaf is cut with `leaf-insert`, targeting that entry. The new leaf is complete-worker-scheduler-k145 and k143 shifted to 110. k143 keeps the aggregate charter and prior failures.
