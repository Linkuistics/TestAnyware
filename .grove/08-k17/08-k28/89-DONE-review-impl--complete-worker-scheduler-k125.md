# complete-worker-scheduler-k125

**Reviews:** complete-worker-scheduler-k122

## Goal

Independently challenge k122's incremental active-workload wait turns and the production compatibility consumer, using current committed source and retained frozen conditions. This seam review does not replace the final bounded-inspection-k28 aggregate review.

## Context

Read experiments/exec-reliability/evidence/workload-turns-k122/report.md, comparison-plan.json, tooling-deviations.md and condition-audit.json. Review controller.py `_WorkloadWait`, `_wait` and its actual `_call` consumer, plus test_controller.py new six-case real-child controls and existing k91 compatibility tests. k91 report and k95 review establish prior entry/return deadline semantics. The report binds final source hashes to inputs-final-2; inputs-final retains the failed first candidate. All six mutants and the RED sample are retained.

## Done when

- Trace the actual consumer and determine whether each advance admits at most one non-reaping observation, never sleeps/signals/reaps/probes, preserves its fixed deadline and bounded absolute wake, rejects entry/return at or after cutoff, propagates observation errors, and leaves terminal calls inert.
- Challenge the synchronous consumer's sleep arithmetic and late-wakeup behavior against k91, including the retained exact-float failure. Check whether new behavior changes normal completion, failure termination or independent safety teardown.
- Audit measurement trust: before/after subjects and artifact hashes, baseline/candidate distinction, added corrective condition, archived initial auditor, mutation isolation and expected failure names, all 60 new child absence journals, two full-run fixture audits, 13 reaped checkers, and 193,709 unchanged prior evidence paths. State limits of injected clock/error controls and source-only conclusions. Use fresh output directories for any new checks; never overwrite retained evidence.
- Verify Taskfile recipes and evidence runner bounds, failure preservation and ownership scope. Challenge material holes in controls and reporting, rather than assuming mutation rejection proves aggregate completion.
- Confirm continuation k124 retains the complete original runtime charter, all historical intermittent failures and outstanding real controls, actual scheduler/teardown overlap, interleaved scheduler comparisons, and final k28 review. k28/k17 remain open; guest acceptance stays 0/10 unavailable.
- Record actionable findings with exact paths/lines and evidence. Create a flat integrate-review-impl sibling before k124 only when findings need reconciliation; create no empty integration. Follow the review kind's independent-provider admission policy; this session had no GROVE_PROVIDER_EVIDENCE and therefore writes no Creator line.

## Decisions (running log)

- 2026-11-15 k125 result: no actionable finding. No integrate-review-impl leaf is created, because there is nothing to reconcile. This was a source-and-evidence review. No new check ran and no retained evidence changed.
  - Consumer trace. `_WorkloadWait.advance` (experiments/exec-reliability/controller.py:167-204) makes at most one `_exited` observation per call. It never sleeps, signals, reaps or probes. It rejects entry at or after the deadline and rejects a return at or after the deadline. It returns `min(deadline, observed + 0.005)`. An `OSError` sets `finished` and propagates, and a later call returns `None`. `_wait` is the only sleeper. It clamps each sleep to `min(0.005, max(0.0, wake - monotonic()))`. A late wake therefore gives a zero sleep, and the next advance re-checks the cutoff. `_call` (controller.py:688) still maps `OSError` to `observation_error`, a normal exit to the leader-reap path, and a miss to `_terminate` followed by `_reap_leader`. Safety teardown is unchanged.
  - Float failure. The retained first candidate (`inputs-final`) differs from `inputs-final-2` only at controller.py:203. The added `min(0.005, ...)` cap restores the original 5 ms bound and the exact-float assertion `sleeps == [0.005] * polls`. Both samples are retained and the corrective condition is a separate directory.
  - Measurement trust. Production `controller.py` and `test_controller.py` hash-match `inputs-final-2`, and all 13 other inputs are byte-equal. `inputs-old` equals the k123 parent controller. Each mutant differs from `inputs-final-2` at exactly one site. Expected failure names match `condition-audit.json` and the JUnit files. The 60 child journals (10 per outcome) all show absence. The full verdict has four checks and both owned-termination fixture journals report absence. All 13 checker invocations across the ten conditions are reaped with no timeout. `prior-after.json` reports 193,709 paths and no change. The Taskfile recipes (Taskfile.yml:1070-1106) match the runner bounds in the report.
  - Limits, not defects. (1) The injected clock and injected `OSError` prove the turn contract only. They do not prove real scheduler behaviour. (2) No mutant targets the `_wait` clamp. The retained exact-float failure and the k91 compatibility test cover it. (3) The six mutant runs select only the six turn controls, not the k91 cases. The full run covers those. (4) This seam does not close the synchronous `_terminate`, `_reap_leader`, final helper capture or worker close waits.
  - Continuation. k124 retains the complete original charter and every log entry. That includes the shared scheduler, teardown overlap, intermittent failures, interleaved comparisons and the final k28 review. k28 and k17 stay open. Guest acceptance stays 0/10 unavailable.
