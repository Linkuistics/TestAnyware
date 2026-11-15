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
