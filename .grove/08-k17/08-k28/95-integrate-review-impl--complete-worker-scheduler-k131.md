# complete-worker-scheduler-k131

**Integrates:** complete-worker-scheduler-k130

## Goal

Triage the findings of complete-worker-scheduler-k130 against the k126 request-transport turns. Apply the real ones narrowly. Leaf k129 keeps concurrent production owners, teardown overlap and the final k28 review.

## Context

Read the k130 Decisions log (findings F1 to F3) and `experiments/exec-reliability/evidence/request-turns-k126/report.md`. The k126 runtime is `advance_query` and `advance_retirement` in `experiments/exec-reliability/worker_capture.py`. The new control is `test_request_turns_yield_with_transport_priority_and_caps` in `experiments/exec-reliability/test_turns.py`.

## Done when

- Each finding has a classification (unclear contract, real issue, visible trade-off or noise) with evidence.
- F1: decide how the control asserts the returned wake values. Add the assertions first, with a red control. Then add one frozen mutant per wake. Candidate mutants: an idle query that returns `now`, and a retirement wake that keeps a past KILL time after the owner is reaped.
- F2: record the idle-sleep latency as a visible trade-off in the k129 handoff. Do not add a control that only restates it.
- F3: add the persistent dispatch `_write(worker.command, ...)` at `worker_capture.py:1112` to the k129 handoff as a remaining blocking boundary. Do not turn it here.
- Keep every prior sample, mutant and receipt unchanged. Use fresh output directories.

## Notes

No guests, privilege, installed changes or unrelated processes. Do not accept this slice as concurrent ownership, teardown overlap or aggregate acceptance. k28 and k17 stay open.

## Decisions (running log)
