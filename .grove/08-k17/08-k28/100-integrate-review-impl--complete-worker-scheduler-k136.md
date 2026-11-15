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

