# complete-worker-scheduler-k110

## Goal

Make worker shutdown incrementally schedulable without changing the declared
shutdown authority, phase receipts or existing synchronous close behavior.

## Context

The full original aggregate charter and running history are preserved in the
flat sibling complete-worker-scheduler-k114. This slice is an independently
usable lifecycle refactor: the production close() consumer drives the same
begin/advance operations that a future shared scheduler can interleave.

Read docs/specs/diagnostic-process-inspection.md, worker-close-k98,
shutdown-phases-k109 and worker-reap-scope-k112 evidence and their reviews.
Existing readiness/request/controller loops and teardown sequencing remain
k114's responsibility. No aggregate acceptance is implied by this slice.

## Done when

- begin_close freezes one shutdown timeline, closes command EOF once and returns
  without a polling/wait/drain loop. advance_close performs at most one exact
  child WNOHANG attempt and due escalation per turn, never sleeps, and returns
  an absolute next wake or terminal None. close drives these same operations.
- Preserve validation, sticky failures, prior finite deadlines and TERM/KILL
  attempts including denial, phase reserve/timing receipts and owner-lifetime
  first-reap exception; repeat begin/advance/close grants no new authority.
- Real owned-worker controls show incremental progress interleaved with independent
  caller work, orderly and failed shutdown, fixed expiry and cleanup audits.
  Exact-clock controls retain equality/late phase accounting. Keep every failed
  sample and old evidence unchanged; freeze inputs and retain hashes/receipts.
- Run repository diagnostic self-check and review-controls through mise/Taskfile.
  Document this bounded result and remaining shared-scheduler work in the
  foundation report. Scheduled review-impl complete-worker-scheduler-k113 owns
  independent challenge; no empty integration leaf is pre-created.

## Notes

No guests, privilege, installed app changes or unrelated process actions.
Ten-clone acceptance is still 0/10 unavailable. k28/k17 stay live. k114 owns
all original runtime/control/final-review obligations, including the shared
scheduler and actual concurrent teardown under immutable deadlines.

## Decisions (running log)

- 2026-11-15: Use flat sibling decomposition because shutdown progression shares
  the existing bounded-inspection context. Ship the nonblocking lifecycle API
  through the existing close consumer now; k114 retains the whole original
  aggregate charter, with k113 reviewing this slice first.
- 2026-11-15: begin_close freezes deadline/history and EOF exactly once;
  advance_close returns a next absolute wake after one nonblocking owner turn.
  A typed private shutdown state holds phase data. Existing close remains the
  compatibility driver; replacing all private loops and wiring actual overlap
  is explicitly retained by k114.
- 2026-11-15: RED retained under incremental-shutdown-k110/red: all 42 new
  incremental real-child cases fail on the absent begin_close API, with exact-child
  cleanup audits retained. Extract the existing phase formulas/receipt assembly;
  each advance owns one _reap call and no sleep, compatibility close drives it.
- 2026-11-15: Frozen green-controls passes 151 cases. Frozen full replay
  passes 796/797; the ignore-term ordering case returns a real ps row
  `30416 30416 ?E` and fails closed as unsupported process state. TERM/KILL
  order, leader reap and worker orderly shutdown succeed in that raw receipt.
  Preserve this failure; do not relax the parser or equate a replay with repair.
- 2026-11-15: Required self-check passes 797 cases and static checks;
  review-controls passes 238. The immutable audit confirms the sole changed
  full-run outcome is the retained ordering/ignore-term case, verifies 453
  named exact-child cleanup receipts and 167,784 unchanged prior evidence paths.
  Document the bounded API and failure limits; leave k113 review and the full
  k114 aggregate live. No aggregate acceptance or parent closure is earned.
