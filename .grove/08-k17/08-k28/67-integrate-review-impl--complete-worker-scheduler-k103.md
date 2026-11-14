# complete-worker-scheduler-k103

**Integrates:** complete-worker-scheduler-k102

## Goal

Triage the findings of the k102 review of the k98 persistent worker close escalation. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the findings in `experiments/exec-reliability/evidence/worker-close-k102/report.md` and in the k102 commit. Re-derive the line positions from the tree as it now stands. The artifact is `Worker.close` and the seven close modes in `experiments/exec-reliability/`, with its evidence in `evidence/worker-close-k98/`. The k28 brief chain and `docs/specs/diagnostic-process-inspection.md` bind the work.

The review found no runtime defect. It found three low-severity test-strength gaps.

- F1: no mode executes denied-signal history or a PID-only target.
- F2: the `expired` mode cannot detect a post-cutoff reap probe.
- F3: no test covers a command-close failure.

Keep the slice narrow. The plan declaration of the orderly-close grace, shared scheduling, overlap with controller teardown and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k101. Preserve every prior evidence byte.

## Done when

- Every k102 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log.
- Each accepted test change has a mutation that fails it, retained as frozen evidence.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k101 and the parent aggregate open.

## Notes

The k102 review was inspection-only. It executed no mutation. Prove each new control with a mutation that the control rejects.
