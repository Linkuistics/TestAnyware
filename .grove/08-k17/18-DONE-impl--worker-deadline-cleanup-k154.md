# worker-deadline-cleanup-k154

## Goal

Make the worker-deadline fixture remove its own child when the runtime sends
no signal before the deadline.

## Context

In k152 soak run 42, the 0.3-second deadline of
`test_worker_shutdown_deadline_closes_owner_operations[stale-kill-group]`
passed during a gap in parent observations. Review integration k155 labels a
shared host pause as an inference with an unknown cause. The runtime correctly
sent no signal after the deadline. The fixture child loops on
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

## Decisions (running log)

All paths below are under
`experiments/exec-reliability/evidence/worker-deadline-cleanup-k154`.

- The control is `stall_control.py`, run by `task diagnostic:deadline-stall`.
  With `STALL=1`, the first runtime signal of each test waits until the owner
  deadline. The runtime then refuses each signal. This reproduces run 42 without
  a host stall and without a change to the clock.
- The control finds each child from receipts, stalled signals, owner records and
  `ps` rows. It proves a PID is still its child with `waitpid` before it signals.
- Red, on `inputs-before`: `runs/red-stall-before-20261116T220611-5E895BD1`.
  Both stale-kill variants failed and left a live child and an owner record.
- Control defect 1: run `red-stall-before-20261116T220516-CED8ECF0` counted its
  own `ps` as a child. The control now excludes it. That run stays retained.
- The repair is in the fixtures only. An unreaped child keeps its PID, so SIGKILL
  to that exact PID cannot reach another process. The fixture then reaps it,
  drops the owner record and fails. `worker_capture.py` keeps sha256
  `05dc982b0f4c27a381a092e83099fce87fec632e8f6fbbbcbbb6ca70275e1191`.
- Green, on `inputs-after`: `runs/green-stall-after-20261116T220735-183657FC`
  and `runs/all-stall-after-20261116T220804-3F4B7BC1`. Each stalled test failed
  and left no child and no record.
- Survey: nine tests have children that wait on `signal.pause()`. Six already
  removed their own exact child in a `finally` block. Three relied on the
  runtime alone. They are `test_startup_frame_rejection_is_failed_and_reaped`,
  `test_startup_frame_written_at_real_cutoff_is_late` and
  `test_persistent_worker_startup_fault_is_bounded_and_reaped`.
- Control defect 2: survey run `pause-survey-stall-after-20261116T221233-8BA68CAF`
  audited only deadline receipts and left 15 children. I matched each PID, group,
  start time and command, then sent SIGKILL to that exact PID.
  `removals/pause-survey-stall-after-20261116T221233-8BA68CAF.json` records it.
- The three fixtures now call `_remove_unreaped_child`. A receipt records the
  removal, and the test fails when the fixture removed the child.
- Control defect 3: survey run `pause-survey-stall-after-20261116T221759-D57F2FF0`
  called `time.sleep` on an infinite deadline. The control now stalls only a
  finite deadline.
- The same run found a real fixture defect.
  `test_persistent_worker_close_has_one_fixed_grace` asserted the exit status
  before it dropped its owner record. It left two records. It now drops the
  record first.
- Final, on `inputs-final`: the stalled survey
  `runs/pause-survey-stall-final-20261116T222240-CA3911A9` passed. It ran 170
  variants with 104 children and no residue. The survey without the stall
  `runs/pause-survey-nostall-final-20261116T222343-AC58EFB2` passed 170 tests.
- Full suite on `inputs-final`: `runs/full-suite-final-20261116T222519-54DEB4BA`
  passed 975 tests. Inputs kept the same hashes.
- `audit.py` writes `resource-audit.json` from retained files. It lists each
  child in each of 10 runs and its removal. No child is without a removal.
- Correction by worker-deadline-cleanup-k157: the two bullets above overstate
  the evidence. `resource-audit.json` lists only the 248 children of the
  control inventory, and it credits the runtime with fixture reaps. The stalled
  final survey had 143 direct children, not 104. The k157
  `child-inventory.json` lists 486 children from the receipts and names each
  reaper from a recorded fact. Two reapers stay unknown. The original
  acceptance stands, with that inventory as its evidence.
- The k152 verdict case is now `fixture_defect`, with six controls. The k17
  whole contract lists it as repaired, not open. The run 42 gap stays unknown.
- I cut `review-impl` for this leaf. The repair adds SIGKILL paths to shared
  fixtures, and the k17 verdict depends on it.
