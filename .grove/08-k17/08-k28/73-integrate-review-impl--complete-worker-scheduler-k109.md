# complete-worker-scheduler-k109

**Integrates:** complete-worker-scheduler-k108

## Goal

Triage the findings of the k108 review of the k104 declared worker shutdown phases. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the report in `experiments/exec-reliability/evidence/shutdown-phases-k108/report.md` and the k108 commit. Re-derive the line positions from the tree as it now stands. The artifact is `Worker.close` in `experiments/exec-reliability/worker_capture.py`. The checker is `test_shutdown.py`, with the prior-deadline controls in `test_worker_capture.py`. The evidence is in `evidence/worker-phases-k104/`. The k28 brief chain and `docs/specs/diagnostic-process-inspection.md` bind the work.

The review found no defect in the shutdown safety behaviour. It found three low-severity findings.

- F1: no control makes the earlier-cutoff reserve `old_end - reap_budget` binding. Every finite-cutoff control clamps `kill_at` to `shutdown_started`. Three single-line mutations survive: drop the reserve, ignore the earlier TERM time, or grant a fresh grace after an earlier TERM.
- F2: the grace-phase `overrun` is true for all 6 ordinary escalations in `full-final-2`. The field cannot separate an on-time escalation from a late one. The assertion at `test_shutdown.py:225` restates the formula.
- F3: the retained evidence samples each of the 12 modes once. No mutant offsets TERM grace or the KILL delay, and the 40-millisecond KILL window is wide.

Keep the slice narrow. The shared earliest-deadline scheduler, overlap with controller teardown, first reap after an expired cutoff and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k107. Preserve every prior evidence byte.

## Done when

- Every k108 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log.
- For F1, add a real-child control whose finite cutoff leaves more than one reap allowance. Retain a failing mutant for each of the three survivors.
- For F2, the receipts separate an on-time escalation from a late one. The specification wording still holds.
- For F3, add a timing-offset mutant for TERM grace and for the KILL delay, or reject the finding with a reason.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k107 and the parent aggregate open.

## Notes

The k108 review was inspection only. It executed no test and no mutation. Prove each new control with a mutation that the control rejects. Use `rel=0` for absolute phase checks on monotonic values.

## Decisions (running log)

