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

## Decisions (running log)

- F1 is a real test sensitivity issue, derived from the review commit 2bcff0cc and the current source. The old controls exercise fragmented valid readiness and expiry, but do not distinguish malformed-frame failures from deadline expiry. Add real-child controls and frozen branch mutants before considering any runtime repair. No runtime defect is established yet.
- The real-time late-frame control will begin a control-channel read before the original cutoff, wait for the child's actual write at that cutoff, and let parsing observe the real clock afterward. This deliberately delayed read models a syscall return stall; it does not change the runtime clock or claim a naturally occurring storage stall. The child will record its write time separately from the strict identity frame.
- Claim to disprove: removing each relevant rejection must make a new control fail. Under doubt-driven-development, decline a fresh read of the same source in favour of frozen executable branch mutants, which directly measure that sensitivity. No cross-model invocation.
- Decoder reachability needs a visible qualification. Three controls with 1,100 or 2,040 nested arrays failed because this native decoder accepts them; restoring the Python recursion limit did not change that. A retained independent probe decoded 5,000 arrays (10,001 bytes) at limits 100/1,000/3,000, beyond the worker's 4,096-byte cap. The final control uses a real child writing 2,202 bytes, the standard library's Python JSON scanner and a temporarily restored 1,000-level recursion limit to exercise the defensive RecursionError conversion. This verifies the guard with an explicit dependency seam, not naturally reachable C-decoder exhaustion. No runtime defect follows from the failed fixtures.
- All preliminary failures, frozen inputs and instrument revisions are retained under `experiments/exec-reliability/evidence/startup-frame-controls-k139/`. A fourth unchanged nesting run followed an edit script's ambiguous-site assertion and a mutant-tool lint failure; the final editing pipeline stops on failure. Final focused condition `candidate-controls-5` passes 14/14.
- `candidate-full-1` retains 920 passes and one existing shutdown failure: `test_declared_shutdown_real_child_phases[blocking-0-denied-kill]`, `test_shutdown.py:298`. Its exact receipt `candidate-full-1/runs/shutdown-blocking-denied-kill-1794738607774520000.json` records a WNOHANG call 0.0003471660311333835 seconds after shutdown end 355886.63276249997; the fixture independently reaped the child. No startup-runtime defect was exposed. Keep the full comparison gate rejected and carry the observation to k137's existing shared-deadline obligation; a later pass cannot repair it. All first-full statics pass. Auditor-only lint initially rejected the long explicit failure key; fixed without changing tests or runtime.
- Final evidence: 14 new controls pass twice; all twelve exact frozen mutants fail at least one new control (none old-only). The second preplanned full suite passes 921/921 and all statics pass; the first red sample and rejected comparison gate remain. The successful 22-condition audit verifies every frozen subject/artifact hash, 306 independent journaled fixture absence checks and 264,064 unchanged prior evidence paths. See `experiments/exec-reliability/evidence/startup-frame-controls-k139/report.md` and `audit-summary.json`.
- Probe task refinement initially failed because Task's embedded shell rejects `set -C`; reconstructed terminal text is retained and labelled. The final task calls `/bin/sh -C`, requires OUT and demonstrably refuses overwrite with unchanged output hash. Both final static followups pass; exact prior Taskfile/instrument revisions remain archived. This is tool repair only. No runtime source changed, no ancestor closes, and the native decoder and delayed-read seam limits remain explicit.
