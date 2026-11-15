# complete-worker-scheduler-k104

## Goal

Declare and validate worker-failure TERM/KILL-reap and orderly-close grace/KILL-reap bounds in Plan and direct Worker use. Apply one fixed close timeline with absolute phase receipts, preserve earlier finite deadlines and attempted signals including denials, and expire healthy grace directly into KILL/reap.

## Context

Read the k28 brief chain and docs/specs/diagnostic-process-inspection.md, k98/k102/k103 worker-close artifacts, k101/k105/k106 bootstrap artifacts, and k96/k99 identity artifacts. The runtime boundary is experiments/exec-reliability/{controller,worker_capture}.py. k106 F5 explicitly requires independent finite-cutoff and denied-attempt controls here. Bootstrap behavior remains covered separately.

The entire original aggregate charter and running history are preserved in flat sibling complete-worker-scheduler-k107. Shared earliest-deadline scheduling, concurrent shutdown/teardown, every unfinished control and final Reviews: bounded-inspection-k28 review remain there. k28/k17 stay open; ten-clone acceptance remains unavailable 0/10 and storage remains k18.

## Done when

- Plan freezes positive numeric worker-failure TERM/KILL-reap and orderly grace/KILL-reap declarations, each <=2 seconds; reject invalid direct Worker declarations before fork and invalid Plan declarations before hashing or output creation.
- Worker.close uses the appropriate declarations. Healthy close gets one EOF grace followed directly by KILL/reap; failure gets TERM grace then KILL/reap. Existing retirement receives no renewed allowance or repeated attempted signals. Earlier owner cutoff always wins.
- Retain absolute phase starts/deadlines/outcomes, overrun/lateness and signal history in receipts, including orderly exit, failure TERM exit, KILL exit, simulated deferred reap and late returns. Preserve bootstrap receipts and idempotent close.
- Retain frozen RED/GREEN controls, independent exact-child audits, full tests/lint/format/strict typing and prior-evidence preservation. Exercise earlier finite cutoffs and attempted/denied TERM/KILL independently of bootstrap. No guest, installed-app, privilege or unrelated-process action.
- Cut independent review-impl of this artifact before k107. Do not certify the aggregate or synchronous close as shared scheduling.

## Decisions (running log)

- 2026-11-15: Flat sibling split ships an independently useful declared shutdown timeline here and retains the original aggregate charter verbatim in k107, because receipts and synchronous close do not establish the shared scheduler or overlap.
- 2026-11-15: Use separate immutable FailureBounds and CloseBounds supplied by Plan or direct Worker. Existing attempt timestamps and finite owner cutoffs constrain all new phases; orderly EOF grace expires straight to KILL. Bootstrap keeps its existing declarations. Independent adversarial challenge is routed to a separate review-impl leaf under Grove; concrete failing controls stay in this session.
- 2026-11-15: Frozen API RED has 45 failures; first full retained 669 passes plus stale pre-ready timing assertion and independent `/bin/ps` capture timeout. Corrected only the obsolete pre-ready assertion, preserved the conservative unknown-inspection failure; next full passed all 679 plus static checks. Five one-site mutants failed 6/2/4/3/2 controls. Reviewing receipts then exposed a genuine phase-entry reporting bug after late wait; strengthened controls failed in three cases before fixing actual-attempt phase entry and prior-KILL grace history. See evidence/worker-phases-k104/report.md for every retained condition and exact limits.
- 2026-11-15: Final receipt implementation passes 679 tests in 63.94s plus Ruff lint/format and strict Pyright; five repeated mutants fail 6/2/4/3/2 of 53 controls. All 15 condition artifact/subject hashes verified, all checker children and 159 shutdown fixture children independently reaped. Prior 151,643 evidence paths identical. Full charter remains in k107; current synchronous loop, overlap and expired-cutoff first reap are explicitly outside this slice. No parent closes. Final source/manifest/lock/Taskfile exactly match frozen inputs-final-2.
