# complete-worker-scheduler-k106

**Integrates:** complete-worker-scheduler-k105

## Goal

Triage the findings of the k105 review of the k101 declared bootstrap bounds. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the findings in `experiments/exec-reliability/evidence/bootstrap-bounds-k105/report.md` and in the k105 commit. Re-derive the line positions from the tree as it now stands. The artifact is `BootstrapBounds`, `Worker.start`, the failed-bootstrap branch of `Worker.close` and `Worker.evidence` in `experiments/exec-reliability/worker_capture.py`. The checker is `test_bootstrap.py` with its evidence in `evidence/bootstrap-bounds-k101/`. The k28 brief chain and `docs/specs/diagnostic-process-inspection.md` bind the work.

The review found no defect in the bounded safety behaviour. It found five low-severity findings.

- F1: `readiness_overrun` and the shutdown `overrun` are true for every ordinary expiry. They do not separate expiry from a late observation.
- F2: no control delivers a valid readiness frame at or after the deadline, so the `>=` branch is untested.
- F3: `overrun` is asserted only as true. A mutation that sets it true always, or swaps the outcome labels, would survive.
- F4: the KILL window (0.065 to 0.16 seconds) and the startup limit (0.75 seconds) are wider than the declared bounds. The `term_deadline_monotonic` receipt equals the schedule it is compared with.
- F5: the earlier-cutoff and signal-history claims have no bootstrap evidence, because the pre-readiness owner deadline is infinity. This finding belongs to the worker-failure and orderly-close work.

Keep the slice narrow. Worker-failure and orderly-close phase declarations, the shared scheduler, overlap with controller teardown and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k104. Preserve every prior evidence byte.

## Done when

- Every k105 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log. Hand F5 to k104 if you accept it as out of scope here.
- Each accepted test change has a mutation that fails it, retained as frozen evidence.
- If F1 needs a runtime change, the receipts separate expiry from lateness and the specification wording still holds.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k104 and the parent aggregate open.

## Notes

The k105 review was inspection only. It executed no test and no mutation. Prove each new control with a mutation that the control rejects.

## Decisions (running log)

- 2026-11-15: F1 is accepted as receipt ambiguity. Keep strict measured `overrun` semantics (including ordinary expiry), as the specification requires every measured overrun to fail; add explicit expiry versus late-valid-frame outcomes and numerical lateness. No tolerance renews authority or makes a failed timeout successful. Preserve the valid-frame observation sample through the error handler, including exact equality.
- 2026-11-15: F2–F4 are accepted as test gaps. Add exact at/beyond-deadline valid-frame controls, both overrun truth values and every shutdown outcome, and independently checked absolute bounds with relative tolerance disabled. Retain a failing single-site mutation for each new assertion family.
- 2026-11-15: F5 is accepted as a scope correction: pre-readiness ownership has no finite request cutoff or prior signals. The k101 report remains unchanged historical evidence; k104 must prove earlier-cutoff and prior-signal preservation for worker failure/orderly close. No aggregate closes here.
- 2026-11-15: F4 also exposed default relative `pytest.approx` tolerances on large monotonic clock values. All new absolute phase checks use rel=0, abs=1e-9. Real signal observation allows 30ms scheduling slack beyond 70ms TERM, and readiness expiry observation allows 50ms; these are disclosed checker allowances, not renewed runtime deadlines. Exact receipt equations and the 500ms readiness, -5ms/+40ms TERM and +20ms shutdown mutations independently constrain declaration use.

- 2026-11-15: Final verification is retained in `experiments/exec-reliability/evidence/bootstrap-bounds-k106/report.md`: 17/17 mutants rejected, repaired bootstrap controls 34/34, final frozen full suite 626/626 plus lint/format/types, 9,517 verified digests across 24 conditions, 142,137 unchanged prior evidence paths. All 39 checker and 192 bootstrap children were reaped; no recorded owned PID remained in the final inventory. Earlier failed tooling/audit/full-suite attempts are retained and disclosed; timing-sensitive existing controls are explicitly handed to k104. F1–F4 close here, F5 and every wider aggregate obligation stay with k104.
