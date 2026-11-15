# startup-frame-controls-k139

**Integrates:** startup-turns-k138

## Goal

Triage the finding of startup-turns-k138 against the k134 startup readiness owner. Apply the real part narrowly. Leaf k137 keeps concurrent production owners, teardown overlap and the final k28 review.

## Context

Read the k138 Decisions log (finding F1) and `experiments/exec-reliability/evidence/startup-turns-k134/report.md`. The owner is `advance_start` in `Worker.start` (`experiments/exec-reliability/worker_capture.py:408-439`). The new controls are `test_startup_turns_*` in `experiments/exec-reliability/test_bootstrap.py`. The older controls are `test_declared_bootstrap_valid_frame_deadline` in the same file. The third older control is `test_persistent_worker_startup_fault_is_bounded_and_reaped` (`experiments/exec-reliability/test_worker_capture.py:2368`).

## Done when

- Classify F1 (contract unclear, real issue, visible trade-off or noise) with evidence. Verify each uncovered branch by reading the source and by a red control, not by inheriting the k138 reading.
- Add red controls first, using a real child that writes the frame. Cover each branch of `advance_start` that rejects a frame. Cover these branches. First, more than 4096 bytes without a newline. Second, EOF before any byte (the child exits at once). Third, EOF after a partial frame. Fourth, bytes after the newline. Fifth, a wrong value in each of `session`, `pid`, `pgid` and `sid`. Sixth, a missing key and an extra key. Seventh, a duplicate key and nesting beyond the decoder limit. Each control asserts the failure text, `readiness_outcome == "failed"` and that the child is reaped within the original bounds.
- Add a control for a valid frame that the child writes at the cutoff, in real time. It must reach the `late` outcome and not the `expired` outcome. The current `late` fault in `test_persistent_worker_startup_fault_is_bounded_and_reaped` cannot do this.
- Add frozen mutants. Candidates: remove the `not chunk` test, drop `tail` from the identity condition, drop one identity field, raise the 4096 limit, and swap `failed` for `expired`. Each mutant must fail at least one new control. A mutant that only an old control rejects must be reported as such.
- Keep every prior sample, mutant and receipt unchanged. Use fresh output directories. Retain every failure.

## Notes

No guests, privilege, installed changes or unrelated processes. Do not accept this slice as concurrent ownership, teardown overlap or aggregate acceptance. k28 and k17 stay open. The original three parent timing failures and the final-parent expired-request failure stay owned by k137. If a red control shows a runtime defect, repair `advance_start` narrowly and record it as a new finding.
