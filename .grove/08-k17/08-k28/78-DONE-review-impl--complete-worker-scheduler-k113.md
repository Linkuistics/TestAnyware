# complete-worker-scheduler-k113

**Reviews:** complete-worker-scheduler-k110

## Goal

Independently challenge incremental worker shutdown against the producer's
narrowed contract and the applicable shutdown/ownership spec.

## Context

Read the k110 producer, its evidence report and the implementation in
experiments/exec-reliability/worker_capture.py plus test_shutdown.py.
The full aggregate remains complete-worker-scheduler-k114; do not certify it
from this refactor. Read worker-reap-scope-k112 and shutdown-phases-k109 for
owner-lifetime first-reap and strict phase timing limits.

## Done when

Trace the real begin/advance/close consumer behavior and challenge nonblocking
progress, one fixed timeline, due signals, prior denied attempts, expired
owner history, idempotence and retained phase evidence. Check real controls
and independent cleanup audit, and attempt a named regression against the
bounded contract. Report findings with evidence, limits and disposition
questions, without fixing them. Reconciliation is earned only by findings.

## Notes

No guests/privilege/installed changes or unrelated processes. Keep every prior
sample. Shared earliest-deadline scheduling, actual cleanup overlap, all
remaining aggregate controls and the final k28 review remain k114's work.

## Findings

Scope: the k110 commit `cbd2aac`. I read the diff, `worker_capture.py` lines 454-645, `test_shutdown.py`, the k110 report and the spec section on worker shutdown.

I ran no tests, because a review may not. Each finding names its evidence and a disposition question.

Held up under challenge: `begin_close` freezes the timeline once. `advance_close` makes one `_reap` call per turn and never sleeps. Terminal turns return `None`. The `_reap` function still holds the owner-lifetime first-reap guard.

Prior denied TERM and KILL still set `kill_at` to the earliest prior attempt. `close()` drives the same two operations. The matrix has 42 incremental cases: three samples for each of 14 modes.

### F1 (medium): the overrun mode removes the nonblocking guard

`test_shutdown.py:196-200` skips the `forbidden_sleep` patch when `mode == "overrun"`. In that mode, `advance_close` can sleep or loop without failing an assertion. Only the poll count (`len(probes) - before <= 1`) still holds.

Named regression: add `while time.monotonic() < state.end: time.sleep(0.001)` at the start of `advance_close`, with no extra poll. By reading the test, all 14 incremental modes still pass.

Question: should the overrun mode inject its 0.27 s delay through the `waitpid` stub only, and keep the sleep guard on for `advance_close`?

### F2 (medium): a late first turn sends TERM and KILL with no grace

In failure mode, `term_sent` is false at the first turn. Suppose the caller's first `advance_close` runs at or after `kill_at`. Then one turn sends TERM and then KILL (`worker_capture.py:546-556`). The worker never gets the declared grace. The receipt records the grace phase as `late` and adds "grace overrun", so the evidence is honest. No incremental control starves the caller, because the test loop sleeps at most 1 ms between turns.

Limit: any slow caller reaches this state, so it is a trade-off as well as a possible defect. The old loop could not reach it without a stalled process.

Question: does the spec allow TERM and KILL in one turn? The spec says an unfinished close enters worker KILL/reap at the grace expiry. If it does allow this, record the limit and add a starved-caller control. If not, `advance_close` must send TERM first and wait one turn.

### F3 (low): `evidence()` reports a residual during shutdown

After `begin_close` and before the final turn, `self.closed` is true and the owner has no reap. Then `evidence()` returns `residual_remedy`. It says the worker closed shutdown authority and the child needs independent reconciliation (`worker_capture.py:676-680`). Meanwhile `shutdown` is `None`. A scheduler that reads evidence between turns sees a residual claim for a child that can still exit cleanly. `test_shutdown.py:216-218` reads evidence only after completion.

Question: should `evidence()` report an explicit in-progress state, or should the contract forbid reading evidence before the terminal turn?

### F4 (low): the exact-clock controls reach the new API only through `close()`

`test_declared_shutdown_exact_escalation_clock` calls `worker.close()` and patches `_reap` and `_signal`. It covers early, equal and 3 ms late accounting for the shared operations. It never calls `begin_close` and `advance_close` directly with a fake clock. It never checks the returned wake value against `kill_at` and `end`. The wake expression at `worker_capture.py:559-563` has no exact-value control.

Question: is the indirect coverage through `close()` enough for k114, or should one control assert the returned wake at the boundaries?

### F5 (medium): the one full-run failure has no attribution

The `green-full` run fails `test_review_group_kill_precedes_inspection[ignore-term]`. My search of earlier `pytest.xml` files found 10 earlier passes of that case. They include bootstrap-bounds-k101, bootstrap-bounds-k106, fork-boundary-k78 and leader-reap-k88. The same case passes in the k110 self-check and review-controls.

The report says the ps row `30416 30416 ?E` is the observed path and the cause is open. It does not show that the k110 change left the timing of that case alone. The controller calls `worker.close()` in its `finally` block (`controller.py:769`).

Limit: the `?E` row is a helper process state seen before worker close, so a causal link is unlikely. Nothing in the evidence rules it out.

Question: should k114 repeat that case N times on the k110 parent and on `cbd2aac`, and keep every sample, before it relies on the unattributed failure?

### Aggregate status

The producer claims no aggregate acceptance, and this review earns none. The leaf k114 keeps the shared scheduler, cleanup overlap and the k28 review.

## Decisions (running log)

- 2026-11-15: The review has five findings, so an `integrate-review-impl` leaf is cut ahead of k114. It carries only this handle.
