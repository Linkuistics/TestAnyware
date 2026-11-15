# complete-worker-scheduler-k112

**Integrates:** complete-worker-scheduler-k111

## Goal

Triage the findings of the k111 review of the k107 first worker reap at an expired cutoff. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the report in `experiments/exec-reliability/evidence/worker-first-reap-review-k111/report.md` and the k111 commit. Re-derive the line positions from the tree as it now stands. The artifact is `_reap` and `_Owner` in `experiments/exec-reliability/worker_capture.py`. The checkers are `test_shutdown.py` and `test_worker_capture.py`.

The review found no defect in the first-reap behaviour. It found two low-severity findings.

- F1: `reap_attempts` counts polls over the life of the owner. A persistent worker that served a request has a high count. If its child exits after the last pre-cutoff poll, the expired close at `worker_capture.py:520` and the final `_reap` at line 1257 make no poll. No control covers this case.
- F2: the three mutants edit only line 130 and line 132. None targets where the counter lives, so a per-cutoff scope has no retained mutant.

Keep the slice narrow. The shared earliest-deadline scheduler, overlap with controller teardown and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k110. Preserve every prior evidence byte.

## Done when

- Every k111 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log.
- For F1, settle the scope of the counter. Either record the limit in the specification and the k110 handoff, or scope the counter to the shutdown cutoff. Add a real-`Worker` control for a polled owner, an exit after the last poll and an expired close.
- For F2, retain one failing mutant for the chosen scope.
- Any scope change keeps the rule that a prior attempt prevents a later poll at or after the same cutoff.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k110 and the parent aggregate open.

## Notes

The k111 review was inspection only. It executed no test and no mutation. Prove each new control with a mutation that the control rejects. Use `rel=0` for absolute phase checks on monotonic values.

## Decisions (running log)

- F1 accepted as an unclear contract and a visible trade-off. `reap_attempts` remains owner-lifetime history, including request polls. A polled worker that exits after its last poll and reaches close after the cutoff remains a failed residual; only a never-polled owner receives the first-attempt exception. Document this limit in the spec and k110 handoff, and verify it with a real persistent worker that serves a request. Per-cutoff redesign belongs to the aggregate scheduler work in k110.
- F2 accepted as a missing scope control. Freeze a close-entry counter-reset mutant and require the polled-owner control to reject it. Keep the existing same-cutoff controls and all earlier evidence unchanged.

- Verification settled: frozen focused 109/109 and full 755/755 pass, with Ruff lint/format and strict Pyright passing. The close-reset mutant fails the required residual-ownership assertion in the one selected real-worker case. `evidence/worker-reap-scope-k112/condition-audit.json` verifies three conditions, six reaped checker processes, 191 exact-child cleanup receipts and 165,755 unchanged prior evidence entries. Report: `experiments/exec-reliability/evidence/worker-reap-scope-k112/report.md`. k110/k28/k17 remain open; no aggregate acceptance is claimed.
