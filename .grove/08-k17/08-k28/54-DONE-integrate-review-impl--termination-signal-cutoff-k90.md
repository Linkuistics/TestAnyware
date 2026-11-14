# termination-signal-cutoff-k90

**Integrates:** complete-worker-scheduler-k89

## Goal

Triage the findings of the k89 review of the k85 workload termination cutoff slice. Apply each real one. Keep the other k85 runtime guards unchanged unless a finding proves them wrong. Keep the k88 aggregate charter unchanged.

## Context

Read the findings from the k89 commit and its task file, not from this body. Reviewed commit: f5a081e (complete-worker-scheduler-k85). Source: `experiments/exec-reliability/controller.py` (`_terminate`), `test_controller.py` termination-cutoff cases, `run_condition.py`. Evidence: `experiments/exec-reliability/evidence/termination-cutoff-k85/`. Contract: `docs/specs/diagnostic-process-inspection.md` lines 201-245.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a trade-off or noise. Record the class in the decision log.
- For each real runtime finding, add a red control first. Then change the runtime. Then retain green evidence in a new directory.
- Preserve all earlier evidence bytes. Run the full diagnostic self-check and retain it, failures included.
- Route anything that belongs to the aggregate scheduler to complete-worker-scheduler-k88 by name. Do not fix it here.

## Decisions (running log)

- Created by the k89 review; all four findings are classified below.

- F1 — real issue: spec lines 224-240 close all signal authority at K-R, not K. Add both-scope equality and reserved-tail controls before changing dispatch; keep original K, PID cap and other guards.
- F2 — real evidence wording issue: k85's six mutations cover exactly three comparison guards and their equality operators. They do not isolate early-bound initialization, the observed-exit short circuit or each observer_ok assignment. Preserve k85 bytes and narrow current documentation rather than invent broader evidence.
- F3 — real receipt issue: record the same monotonic sample used by signal admission. Add a stepping-clock red control before runtime change; sampling still cannot prevent preemption before syscall.
- F4 — real runner robustness issue: reject unknown or extra modes before creating output or dispatching checkers. Preserve the historical k85 runner; use a k90 successor for the reusable task.
- Aggregate routing: expired-deadline mandatory first nonblocking reap attempt belongs to complete-worker-scheduler-k88, along with all existing shared-scheduler/probe/reap work. Its charter is unchanged; no ancestor closes here.

- Verification: baseline red-1 failed exactly eight of twenty controls before runtime edits. Fixed green-1/green-2 pass twenty; restoring only K admission fails six, restoring only the earlier receipt sample fails two. The exact failure sets are disjoint and reproduce the baseline union. Missing/unknown/extra modes fail three controls on the old runner and pass three on the successor.
- Full diagnostic self-check full-1: 454 tests plus lint, formatting and strict types pass. All checker receipts are reaped without timeout; before/after measured subject hashes match. This passing sample does not resolve earlier intermittent fixture failures.
- Integrity: audit.json verifies 101,691 prior evidence files unchanged and the exact retained condition identities/artifact hashes. A failed first auditor attempt (root Taskfile compared with its pre-fix hash) is retained; the correction uses the frozen red Taskfile. Green-1’s exact pre-format runner is retained and hash-verified. Mode-only invocations lack a pre-dispatch test-source hash sample; raw streams/JUnit are retained and this limit is explicit.
- Delivered report: experiments/exec-reliability/evidence/termination-cutoff-k90/report.md. Current foundation wording narrows k85 mutation claims without changing k85 evidence. No ADR changes: the K-R contract was already explicit. No parent closes because complete-worker-scheduler-k88 remains live.
