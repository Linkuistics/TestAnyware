# sticky-inspection-failure-k45

**Reviews:** sticky-inspection-failure-k43

## Goal
Contest the committed sticky-inspection-failure slice before the worker/scheduler
implementation builds on it.

## Context
Read the producer commit, ancestor contracts and the process-inspection spec.
The artifact is controller.py's execution-owned inspection state and its call/
termination/teardown plumbing, seven controls in test_controller.py, the k43
frozen prior/current snapshots and replays, final self-check, independent audit
and foundation report. This is a bounded slice; k44 retains the full remaining
worker/scheduler contract and its final full-subsystem review.

## Done when
- Review the actual runtime paths for every unknown observation, first-failure
  provenance, no later local query/probe, stopped workload and fresh-run isolation.
  Contest whether valid live classification remains distinct from inspector failure.
- Check that all acquired resources' independent reverse safety commands still
  execute, including real timed-out cleanup signals/reaping, and that disposal
  certainty cannot recover after local inspection becomes unavailable.
- Independently assess test seams, prior/current per-item comparison, raw retained
  receipts, input/artifact integrity, exact-PID and owned-resource/scratch audits,
  and whether the report distinguishes adapters, actual observations and limits.
- Record substantive findings against the committed artifact. If reconciliation
  is warranted, insert integrate-review-impl sticky-inspection-failure before
  k44, carrying this review handle rather than transcribing findings as its
  charter. If no actionable findings remain, retire without an empty integration.

## Decisions (running log)
Reviewed commit 35d806d by inspection only. No tests, builds or lint were run.

Checked against the committed artifact:
- Only `_InspectionState.snapshot` (controller.py:344) reaches `_group_snapshot`, through `_final_snapshot`. The two callers are controller.py:443 and :514.
- A latched failure returns a synthetic `not_attempted` observation. No `ps` or `killpg` query follows it.
- `_terminate` sends TERM and KILL, and reaps, regardless of the latch. The teardown loop in `execute` still runs reversed cleanup, dispose and inspect.
- A valid `live` table does not latch. Only `unknown` does.
- `inspection_state` is created inside `execute`, so a fresh run starts clean.
- k43-audit-01 `controls.json` shows live, mixed, invalid and missing `ps` selectors behave as the audit assumes.
- The audit asserts the exact failing set of each prior replay. Only `controller.py` differs between the frozen prior and current inputs.
- The report names the adapters. It says they alter classification and do not show a real malformed `ps`.

Finding 1 (medium, controller.py:314-332 and :361-363): the synthetic `unknown` from `_final_snapshot` also latches.
- Both "no observed leader exit before inspection cutoff" and "inspection cutoff exhausted before final query" make no query. They are not inspector failures.
- The run then records `inspection_failure` as if the local inspector failed, and every later call reports "local inspection unavailable after <call>".
- Safety direction is correct, because disposal stays uncertain. The cost is lost real observations of later calls, and a mislabelled cause.
- No test pins either behaviour. `test_review_final_query_requires_observed_exit` (test_controller.py:188) never asserts the latch state.
- Decision needed: either latch only on a real query failure, or record a failure kind so the report separates "inspector failed" from "exit not observed".

Finding 2 (low, test seam): the sticky controls inject `unknown` through a `_group_snapshot` monkeypatch after one real query. No test latches on a real `ps` timeout, usage error or foreign table and then checks later calls. The report discloses the adapter. Add one real-failure control if the integrator reworks Finding 1.

Finding 3 (low, replay quality): in k43-previous-02, `test_review_live_table_does_not_poison_later_inspection` fails on the prior controller. The cause is probably the missing `inspection_failure` key, not a behavioural difference. The comparison shows failure but does not show why.

Not found: no violation of reverse safety commands, reaping, fresh-run isolation or audit integrity.

Findings 1 and 2 warrant reconciliation. Inserted k46 (integrate-review-impl) before k44. Finding 3 is advisory.
