# complete-worker-scheduler-k97

**Reviews:** complete-worker-scheduler-k94

## Goal

Independently challenge the persistent inspection worker lifecycle and its controller integration against the narrowed k94 contract. Report findings with exact evidence; do not implement fixes in this review.

## Context

Read the committed k94 producer, the k28 brief chain and `docs/specs/diagnostic-process-inspection.md`. The artifact comprises `experiments/exec-reliability/{controller,inspection,worker_capture}.py`, their tests, root Taskfile and `experiments/exec-reliability/evidence/persistent-worker-k94/`. The report and frozen source copies retain both successful and failed measurements. In particular inspect full-1 and full-4 alongside the unchanged isolated full-5; scheduling differences do not establish causation or repair.

The review's bounded contract is one admitted direct-child worker ready within one second before resource acquisition, sequential framed requests through that same child, sticky failure without replacement/retry, fixed shutdown authority with retained exact-child residuals, and public safety teardown despite inspection failure. Shared earliest-deadline scheduling, complete workload/run/epoch authentication and overlapping shutdown/teardown remain explicit k96 requirements. k28/k17/root remain open. Do not infer aggregate acceptance from this slice.

## Questions to challenge

- Trace initial and immediate-prefork admission, child state/descriptor reset, readiness identity/timing and all startup/preparation failure exits. Determine whether resource acquisition can precede valid readiness or any child/descriptor escapes ownership.
- Inspect persistent command/control/result framing across successive requests, including completion arriving before the full result, duplicate/missing/wrong completion, stale tokens, partial/overflow/backpressure, late helper identity and queued input. Check byte/turn bounds and whether one response can contaminate another.
- Trace all service and request failure paths. Challenge the absence of retry/replacement, immutable already-expired authority, repeated close, worker death with known or unknown helpers, and the meaning of every reap/residual claim. Distinguish helper identity from proof of helper death.
- Follow controller acquisition, workloads and every safety path, including unexpected safety-adapter exceptions. Check that explicit worker ownership neither suppresses required safety work nor grants worker-origin workload signal authority.
- Assess whether the real controls and independent exact-child/process observations substantiate the lifecycle claims and reveal meaningful regressions. Reconcile the retained intermittent failures rather than treating a later pass as proof of repair. Use bounded counterexamples or mutations when needed and freeze every measured subject before and after.

## Done when

- Review the committed artifact and its relevant ownership/call paths; enumerate changed waits, signal/reap authority and failure exits with source evidence and coverage limitations.
- Retain a readable findings report and any frozen bounded verification records, preserving all prior evidence bytes. Separate actionable contract violations from the explicitly owned k96 remainder and unestablished timing mechanisms.
- Follow review-impl's ordinary findings/reconciliation route; create an integrate-review-impl step only when findings earn it, before the next live sibling.
