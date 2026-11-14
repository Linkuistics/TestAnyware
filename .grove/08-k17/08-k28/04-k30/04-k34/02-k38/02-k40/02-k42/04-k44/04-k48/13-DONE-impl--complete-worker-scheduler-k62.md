# complete-worker-scheduler-k62

## Goal

Replace the interim helper Popen boundary with explicit raw fork, child reset and exact-child ownership. Ship this independently useful runtime behavior in the current per-query worker without claiming the persistent scheduler complete.

## Context

The full original contract is carried, unchanged, by complete-worker-scheduler-k64. Read k48 and the spec for the aggregate requirements. k63 reconciled the admission receipt boundary. Raw helpers must not run inherited Python handlers or exit hooks, leak evidence/control descriptors through exec, or introduce implicit Popen polling/waits.

## Done when

- The actual inspection helper uses raw fork/exec and os._exit on every child failure path. Admission still precedes creation, with signals blocked through child reset and the parent's mask restored. Reset handlers, wakeup FD, descriptor allowlist, local owner state and session before exec.
- Register the exact child as an explicit owner; capture bounded stdout/stderr and use only exact-child WNOHANG reaping and conditional exact-PID KILL under the caller's existing fixed deadline. No Popen construction, destructor polling, context-manager wait or implicit cleanup remains for this helper.
- Retain real controls for inherited handlers/hooks/wakeup/evidence descriptors, closed standard descriptors, fork error, reset/exec failure, and normal/slow/overflow helper capture. Check the controller path and interim worker transport remain green. Tests distinguish real process controls from error adapters.
- Freeze subjects and bounds for retained RED/GREEN evidence, preserve all earlier evidence byte-for-byte and document limits. No VM/app changes or privilege acquisition. Ten-clone acceptance stays unavailable 0/10.
- Cut a separate review-impl of this fork/ownership boundary before the complete remainder. Keep all aggregate ancestors and complete-worker-scheduler-k64 live.

## Decisions (running log)

- 2026-11-14: Flat sibling split ships an independently verifiable helper-fork behavior rather than another receipt-only adjustment. complete-worker-scheduler-k64 owns EVERY original remaining scheduler requirement, all outstanding controls and the final bounded-inspection review. This leaf supplies no authenticated/gated helper identity or controller-owned helper reap acknowledgement; worker failure can still leave unknown creation. Parent death does not prove helper death.

- 2026-11-14: Register raw helper PID and creation time before restoring the parent signal mask; recover this owner on a parent-side exception. Pipe endpoints are normalized above stdio before fork. Child reset/exec exceptions terminate with os._exit(70). With no child-error channel, a missing executable now yields a known reaped child with status 70, not a claimed ENOENT/no-child receipt; explicit fork refusals retain parent errno. Exit 70 cannot distinguish reset failure, exec failure, or an executable choosing 70. Gated identity and richer failure publication remain k64 work.

- 2026-11-14: Preserve stdin across exec explicitly after reset reopens /dev/null: os.open defaults to non-inheritable. Added preexec inheritability and actual postexec stdin-read controls; stdio-red01 catches both failures. Retain child reset observations alongside capture receipts so the review can inspect actual state rather than only assertion results.

- 2026-11-14: Settled raw-fork slice: final green03 passes lint/format/strict types and all 258 tests; retain red01, green01, green02 and stdio-red01 as well. Every run has matching before/after subject hashes and matching source snapshots. All 50,053 pre-existing evidence files (including ignored) remain byte-identical. See experiments/exec-reliability/evidence/raw-helper-fork-k62/report.md and its inventories for methods, adapters and limits. Full scheduler work stays in k64; no aggregate ancestor closes.
