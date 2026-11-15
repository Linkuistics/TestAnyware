# complete-worker-scheduler-k136

**Integrates:** complete-worker-scheduler-k135

## Goal

Triage the findings of complete-worker-scheduler-k135 against the k132 readiness wakes. Apply the real ones narrowly. Leaf k134 keeps concurrent production owners, teardown overlap and the final k28 review.

## Context

Read the k135 Decisions log (findings F1 and F2) and `experiments/exec-reliability/evidence/readiness-wakes-k132/report.md`. The driver is `run_turns` in `experiments/exec-reliability/turns.py`. The registrations are `query_readiness` and `retirement_readiness` in `experiments/exec-reliability/worker_capture.py`. The controls are in `experiments/exec-reliability/test_turns.py`.

## Done when

- Each finding has a classification (unclear contract, real issue, visible trade-off or noise) with evidence.
- F1: decide the contract for a failing readiness callable or `select`. Candidates: the driver catches `OSError` and `ValueError` from interest sampling and `select`, then falls back to the bounded sleep so the owners still run. The owners then meet any bad descriptor in their own nonblocking I/O. Add a red control first. Cover a safety retirement turn, so the control shows that `_reap`, the KILL and the receipt still happen. Add one frozen mutant that removes the fallback.
- F2: add controls that read the registrations after interest closure. Cover a stream at EOF, a finished command write and a cleanup reader at EOF with the owner still alive. Each must return an empty interest. Add one frozen mutant per closure guard. Candidate mutants: keep `control_read` registered after `cleanup_open` is false, and skip `del streams[fd]` for the registration.
- Keep every prior sample, mutant and receipt unchanged. Use fresh output directories. Retain every failure.

## Notes

No guests, privilege, installed changes or unrelated processes. Do not accept this slice as concurrent ownership, teardown overlap or aggregate acceptance. k28 and k17 stay open. The original three parent timing failures and the final-parent expired-request failure stay owned by k134.

## Decisions (running log)

- F1 is a real issue, verified against the k135 commit and current `run_turns`/retirement source: advisory interest sampling or `select` can escape the `finally` retirement before KILL, reap and receipt. Catch only `OSError` and `ValueError` in advisory sampling/waiting and use the existing bounded timer sleep; keep `advance` exceptions propagating. Verify with exact-clock production retirement callbacks, simulated fork/wait/signal authority, both error classes at both advisory sites, and a frozen fallback-removal mutant.
- F2 is a real test-sensitivity gap, rather than a demonstrated runtime bug. The current stream deletion, command-empty branch and cleanup-open guard implement closure. Add direct post-closure observations while the owner remains live and one frozen mutation per guard. No redesign or production concurrency claim follows from these repairs.
- Graph Verify lookup found no symbols for `turns.py`. Coverage generation `2026-11-15T02:19:52Z` reports runtime/tests/evidence tools untracked or metadata changed. Direct source reads are the fallback evidence; no exhaustive graph claim is made.

- Initial RED is four advisory exceptions/three closure passes. First candidate completes retirement but fails four lower-bound literal comparisons (`10.9625 <= 10.962499999999999`). Preserve this failed reading and its frozen inputs/instruments. Calculate the test lower bound from declared inputs `10.95 + 0.05 / 4`, keeping the original cutoff and reserve; repeat RED/GREEN with new snapshots.

- F1 is repaired by the narrow advisory fallback; final RED repeats four failures, both final focused runs pass all seven cases, and the frozen fallback-removal mutant fails all four advisory cases. The real retirement callback reaches simulated KILL/reap/receipt within its original cutoff; owner `advance` errors still propagate under the existing control.
- F2 is covered by direct post-write/post-EOF registrations with the owner still live. Frozen stream, command-empty and cleanup-open mutants reject in two, one and one cases respectively. Existing registration code needs no change.
- Both final full suites pass 903 tests plus Ruff, formatting and strict Pyright. The frozen reviewed driver with the same final tests passes 899 and fails exactly the four new advisory cases. The item-by-item/hash auditor passes; all checker children are reaped, no checker times out, all 27 recorded exact-fixture audits show absence, and all 248,391 prior evidence paths are unchanged. The original floating-point assertion failure and all earlier failures remain retained.
- Final artifact: `experiments/exec-reliability/evidence/readiness-review-k136/report.md`. k134 retains concurrent production ownership, teardown overlap, every outstanding control/failure and final k28 review. No ancestor closes; aggregate acceptance remains false and guest acceptance remains 0/10 unavailable. No ADR change is needed for this narrow repair to the existing advisory-readiness contract.
