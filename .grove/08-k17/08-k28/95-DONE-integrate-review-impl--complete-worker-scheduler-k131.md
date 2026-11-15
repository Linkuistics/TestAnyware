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

- 2026-11-15: Read k130 from review commit d6b2dea2 and verified against the
  active callbacks. F1 is a real test-sensitivity issue, not a runtime defect:
  the real control returns wakes without checking them. Add wake assertions
  there and deterministic callback controls with an idle poll and cleanup kept
  open after reap/ownership loss past the original KILL time. Frozen mutations
  will supply the red controls before crediting green; production is unchanged.
  F2 is a visible trade-off: a zero-timeout poll followed by driver sleep can add
  up to 5 ms observation latency. F3 is a remaining blocking boundary: persistent
  command `_write` precedes the driver and may wait through the query deadline.
  Both are handed to k129 without expanding this slice into scheduler redesign.

- 2026-11-15: Assertions were added before measurement. Frozen `red-idle`
  rejects idle-spin in all four real transport cases and both exact-clock cases;
  `red-stale` rejects retaining the old KILL wake after reap/ECHILD in both
  exact-clock cases. Unmodified production passes all six focused cases.
  Additional ready-delay and slow-retirement mutations are rejected. Each uses
  a fresh input/output directory under evidence/request-wakes-k131; every
  checker reaped and externally audited its exact owned fixtures as absent.
  F2/F3 now appear explicitly in k129's opening handoff. No latency restatement
  control or production dispatch rewrite was added.

- 2026-11-15: The frozen complete diagnostic suite passed all 881 cases with
  Ruff/format/Pyright green; the separate static condition and final evidence
  tools checks are green. The audit maps all focused cases into that suite,
  retains every named mutant failure, confirms one runtime-file mutation hunk
  per mutant and validates artifact hashes and exact fixture cleanup.
  Preservation compares 214670 prior paths with no change. Report:
  `experiments/exec-reliability/evidence/request-wakes-k131/report.md`.
  No runtime repair or new ADR is warranted by these findings. All Done when
  items for this integration are met; k129, k28 and k17 remain open with their
  existing aggregate obligations and historical failed samples intact.
