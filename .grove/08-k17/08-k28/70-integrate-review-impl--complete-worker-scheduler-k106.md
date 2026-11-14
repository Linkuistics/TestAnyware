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

