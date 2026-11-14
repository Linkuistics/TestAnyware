# complete-worker-scheduler-k99

**Reviews:** complete-worker-scheduler-k96

## Goal

Independently challenge the persistent request identity envelope and controller scope against the narrowed k96 contract. Report findings with source and retained evidence. Do not implement fixes in this review.

## Context

Read the committed k96 producer, the k28 brief chain, `docs/specs/diagnostic-process-inspection.md` and `experiments/exec-reliability/evidence/request-identity-k96/report.md`. The artifact is `experiments/exec-reliability/{controller,inspection,worker_capture}.py`, the controller and worker tests, root Taskfile and the frozen request-identity evidence. Read retained failed runs as well as final runs; a subsequent pass alone does not explain a historical failure.

The bounded contract binds persistent commands and readiness/intent/creation/explicit-fork-failure/completion/terminal messages to run ID, worker session and actual PID, unique request, workload PID/PGID, signal epoch, phase and immutable deadline. Malformed or mismatched identities must fail sticky, with no retry/replacement and no helper-obligation retirement from an invalid terminal receipt. Matching sequential requests must remain usable. Controller scopes must describe the actual run, pinned workload and attempted termination signals, including denied attempts.

Shared scheduling, cutoff closure of epochs, overlapping shutdown/teardown, remaining aggregate controls and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k98. The standalone one-shot compatibility transport is outside this slice. k28/k17 remain open. Distinguish defects in this bounded contract from that explicit remainder.

## Questions to challenge

- Trace envelope construction, service admission and every persistent message acceptance path. Can a missing, extra, mistyped, duplicate or stale identity be accepted, or an envelope describe a different worker, request or workload? Check exact types before equality, finite deadlines, run/session pinning and request reuse.
- Trace helper creation and explicit fork-failure publication, terminal receipt validation and ledger retirement. Can an invalid response erase a known or unknown helper obligation, or lose cleanup-only identity evidence? Separate receipt identity, actual helper death and independent reap acknowledgment.
- Follow the controller's normal and termination paths. Does every persistent final query receive the correct run/workload/phase/deadline and signal-attempt epoch? Challenge denied signal attempts and compatibility call paths. Do not infer cutoff closure from an epoch number alone.
- Assess the real-transport test instruments, injection markers, exact-child and process absence audits, frozen subjects and checker bounds. Determine whether the final mutations demonstrate the rejection behavior claimed, and whether fixture failures, buffering or test-local ledger cleanup conceal a bounded runtime defect.
- Inspect sequential request reuse and sticky failure with the new envelopes. Can partial, late or conflicting control/result traffic contaminate the next request or leave an unreported ownership residual within this slice?

## Done when

- Review the committed artifact and relevant callers/callees with source evidence and disclosed coverage limits. Enumerate the identity checks, acceptance order and helper-ledger consequences that support material claims.
- Retain a readable findings report and any frozen bounded verification records, preserving every prior evidence byte. Reconcile recorded failures without inventing causation or treating unavailable privileged controls as executed.
- Use review-impl's findings/reconciliation route. Cut an integrate-review-impl only if actionable findings earn it, before the next live sibling. Leave complete-worker-scheduler-k98 and the parent aggregate open.


## Goal



## Context

## Done when

## Notes

## Decisions (running log)

- 2026-11-15: The review of k96 found no runtime defect in the bounded contract. It found two low-severity test-strength gaps, so it cut `complete-worker-scheduler-k100` (integrate-review-impl) ahead of k98.
- 2026-11-15: The report is `experiments/exec-reliability/evidence/request-identity-k99/report.md`. It lists the identity checks, the acceptance order, the helper-ledger consequences and the controller paths. It reconciles the retained k96 runs.
- 2026-11-15: F1. No test covers worker-side token replay or the worker-side run ID pin. The mutation-no-check-final run cannot show those checks.
- 2026-11-15: F2. The ready, intent, created and fork-failure damage cases do not assert the conservative ledger state. The retained receipts show the state is correct today.
- 2026-11-15: The review was inspection-only. It ran no test, build, lint or format command and executed no counterexample or mutation, so it froze no new subject. It preserved every prior evidence byte.
- 2026-11-15: Observations, not findings. The epoch counts a `deadline_exhausted` event, the worker run ID is trust-on-first-use, and cleanup drops later valid created frames after a damaged one. k98 settles the epoch wording.
- 2026-11-15: Coverage limit. The review did not read every test body and did not digest dependency implementations beyond the named subjects. The privileged and ten-clone controls were unavailable and count as not executed.
