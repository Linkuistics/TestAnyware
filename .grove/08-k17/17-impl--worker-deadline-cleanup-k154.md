# worker-deadline-cleanup-k154

## Goal

Make the worker-deadline fixture remove its own child when the runtime sends
no signal before the deadline.

## Context

In k152 soak run 42, a host-wide stall of about 160 milliseconds crossed the
0.3-second deadline of
`test_worker_shutdown_deadline_closes_owner_operations[stale-kill-group]`. The
runtime correctly sent no signal after the deadline. The fixture child loops on
`signal.pause()`.

The `finally` block in
`experiments/exec-reliability/test_worker_capture.py` polls for a reap for one
second. It removes the owner record only after a reap, and it never signals the
child. So the child stayed alive as PID 4100, with parent PID 1. Its owner
record stayed in `worker_capture._owners`, and
`test_review_worker_rejects_fork_before_creation` failed on that record.

The k152 report and verdict case `host-stall-stale-kill-k152` hold the
timestamps. The evidence is in
`experiments/exec-reliability/evidence/historical-soak-k152/soak/runs/042-full-idle-ac1a6eb7`.

## Done when

- A frozen failing control reproduces the leak without a real host stall. For
  example, it delays the clock past the deadline before the runtime signals.
- After the repair, the same control passes. The fixture removes the exact
  child and its owner record, and the test still fails.
- The repair changes only the fixture. The runtime keeps its fail-closed rule
  after the deadline.
- Each other fixture that waits on `signal.pause()` gets the same check, or the
  log states why it cannot leak.
- The resource audit of each control run lists each child and its removal.
- The k152 verdict case and the handoff reflect the result.

## Notes

Do not raise the 0.3-second deadline to hide the stall. Cleanup outside the
runtime must address only the exact child that the test created. The k152 soak
stays as it ran. k18 owns stalled storage.
