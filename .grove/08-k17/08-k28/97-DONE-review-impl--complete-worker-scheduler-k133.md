# complete-worker-scheduler-k133

**Reviews:** complete-worker-scheduler-k129


## Goal

Adversarially review the bounded persistent command-dispatch change and its
evidence before the aggregate scheduler resumes. Determine whether this seam is
correct and whether its claimed completion is justified with the full comparison
gate rejected.



## Context

Read the producer commit and
`experiments/exec-reliability/evidence/command-dispatch-k129/report.md`, then
inspect the production callback, actual-write identity mutation fixture, exact
clock controls, frozen comparisons and evidence auditor. The change moves the
parent command write into the existing request turn; it does not implement
concurrent owner scheduling or teardown overlap.

The final interleaved suites retain a standalone snapshot timeout in candidate 1
and an expired-request residual-remedy failure in parent 2. Earlier runs also
retain the postcutoff shutdown observation and missed command-mutation hooks.
The provenance audit verifies integrity but deliberately exits nonzero for the
unexpected standalone timeout. Do not substitute later passes for explanations
or widen the predeclared historical allowances. The full original aggregate
charter and every unresolved failure remain live in
complete-worker-scheduler-k132; k28/k17 and ten-clone acceptance remain open.

## Done when

- Review the single-write bound, partial progress, backpressure wake, initial and
  post-observation cutoff checks, competing safety priority, terminal behavior,
  sticky failure and conservative creation ownership against actual code.
- Challenge whether the controls and rejecting mutants establish those claims,
  including the adapted command mutation hook and real worker/helper absence
  audits. Verify frozen subject equality and preservation rather than relying on
  aggregate pass counts.
- Assess the retained comparison rejection and the split's remaining obligations.
  Record actionable findings with source/evidence anchors; cut the appropriate
  integration step ahead of the live aggregate if findings merit action.

## Notes

## Decisions (running log)

- 2026-11-15: Reviewed k129 by inspection only. Verdict: the seam is correct, and its narrowed claim is justified. The claim covers the persistent command write only. The retained comparison rejection stays open in k132.
- 2026-11-15: Single-write bound holds. `advance_query` in `worker_capture.py` does at most one `os.write` per call, and the command is capped at 4096 bytes before dispatch. `inspection.capture_pipe` sets the write end nonblocking (`inspection.py:215`), so the write cannot block. A partial count keeps the tail in `command`. `BlockingIOError` returns `min(query_deadline, now + 0.005)`. EPIPE and zero progress raise and make the worker failure sticky.
- 2026-11-15: Cutoff checks hold. One check precedes `_reap` and one follows it, immediately before the write. The `cutoff` and `reap-cutoff` controls in `test_turns.py` fail on the parent, and the `mutant-cutoff-write` mutant rejects removal of the second check.
- 2026-11-15: Frozen subject equality verified. The sha256 of `worker_capture.py`, `turns.py`, `inspection.py`, `test_turns.py` and `test_worker_capture.py` match `inputs-candidate-verified`. The parent inputs differ from the candidate only in `worker_capture.py`, and equal the pre-commit source.
- 2026-11-15: Finding F1 (minor, no action): `dispatched` is stamped when the command is built, not at the first write. Worker stamps stay valid because the lower bound is only looser. The receipt field `dispatch_monotonic` now means "built". k132 should stamp it at the first write attempt if it needs a tight bound.
- 2026-11-15: Finding F2 (limit, no action): production registers one nonsafety `Turn(advance_query)` at `worker_capture.py:1283`. Competing safety priority is proven only against the synthetic competitor in `test_command_dispatch_turns_yield_and_stop`. This matches the k128 caveat that k132 must verify through competing owners.
- 2026-11-15: Finding F3 (documentation, no action): `98-impl--complete-worker-scheduler-k132.md` line 15 still lists the blocking command `_write` as open work. The k129 handoff at line 232 corrects it. An implementer must read both.
- 2026-11-15: Retained rejection assessed. The standalone `test_review_group_snapshot_uses_real_worker` timeout runs with `worker=None`, so the new branch did not execute. The cause is unexplained and stays in k132. The auditor correctly exits nonzero and widens no allowance. No integration leaf is cut, because no finding needs a code or evidence change.
