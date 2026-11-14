# complete-worker-scheduler-k100

**Integrates:** complete-worker-scheduler-k99

## Goal

Triage the findings of the k99 review of the k96 request identity envelope. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the findings in `experiments/exec-reliability/evidence/request-identity-k99/report.md` and in the k99 commit. Re-derive the line positions from the tree as it now stands. The artifact is the k96 code and tests in `experiments/exec-reliability/` and its evidence in `evidence/request-identity-k96/`. The k28 brief chain and `docs/specs/diagnostic-process-inspection.md` bind the work.

Keep the slice narrow. Shared scheduling, cutoff closure of epochs, overlapping shutdown and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k98. The one-shot compatibility transport stays out of scope. Preserve every prior evidence byte.

## Done when

- Every k99 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log.
- Each accepted test change has a mutation that fails it, retained as frozen evidence.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k98 and the parent aggregate open.

## Decisions (running log)
