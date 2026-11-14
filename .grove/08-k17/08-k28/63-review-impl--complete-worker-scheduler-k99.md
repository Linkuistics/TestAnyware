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
