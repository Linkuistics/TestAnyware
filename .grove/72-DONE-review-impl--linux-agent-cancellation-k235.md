# linux-agent-cancellation-k235

**Reviews:** linux-agent-cancellation-k220

## Goal

Try to break the Linux agent change of linux-agent-cancellation-k220. The
macOS leaf macos-agent-cancellation-k221 and the Host CLI leaf
host-cli-control-k222 build on its wire shapes next. Find each request that blocks
another request. Find each cancellation, exit or shutdown order that gives a
record the kernel did not produce. Find each download that sends a byte past
its declared length or reports success for wrong bytes.

## Context

- Spec `docs/specs/guest-command-lifecycle.md`: sections "Arbitration",
  "Termination after a timeout or a cancellation", "Agent shutdown",
  "Cancellation and CLI interruption", "Responsiveness", "Exec record",
  "Evidence transfer", "Bounds" and "Test seams and required controls".
- ADR-0024, ADR-0025 and ADR-0026.
- The contract of k220 is its `Done when` list and its notes. The notes
  carry finding D5 of macos-agent-lifecycle-k234, the `poll` overflow.
- The code: `agents/linux/testanyware_agent/server.py`, `exec_registry.py`,
  `exec_owner.py`, `system_endpoints.py` and the lock in `accessibility.py`.
- The controls: `agents/linux/tests/test_agent_process.py`,
  `test_exec_cancellation.py`, `test_exec_registry.py`,
  `test_responsiveness.py`, `test_file_transfer.py` and the changed seams in
  `test_exec_lifecycle.py`. `task agent:linux:test` runs them.
- Retained evidence: `experiments/exec-reliability/evidence/linux-agent-cancellation-k220`.
- Documents: `docs/architecture/agent-protocol.md` and
  `docs/components/agents-linux.md`.

## Specific doubts

- One owner must handle the leader exit, the deadline and a cancellation in
  order. Check that a cancellation never wins over a leader that has exited.
  Check that a timeout never becomes a cancellation, and the reverse.
- The cancellation reaches the owner through a pipe. Compare every path that
  closes the pipe with every path that writes to it. Include the
  before-start path, a tombstoned id and an owner fault.
- Health must never take the AT-SPI lock and must still report
  `accessible`. Check what health reports, when, and whether any AT-SPI call
  can run outside the lock.
- Retention: 300 s, 64 at most, the oldest first, never a running command.
  Check eviction, tombstones and duplicate ids across every registry entry
  point.
- Shutdown at SIGTERM: check the signal handler, the order of admission,
  cancellation and socket close, and the bound on the whole exit. Check a
  leader whose status stays unavailable.
- Timeouts at the edge of the accepted range: huge integers, floats, and the
  JSON literals that Python accepts.
- Download: one handle, exactly the declared length, and a short body on
  truncation. Check whether the client can always tell a short body from a
  complete one.
- The old-fail controls must fail on the old path for the stated reason, and
  every control must be able to fail on the new code.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings, not fixes.

## Notes

Review the producer commit of linux-agent-cancellation-k220 against the
current source. Do not replace a golden, and do not change the global CLI or
app.

## Decisions (running log)

- Review producer commit `b41c7492` against the current source by inspection
  only. No test, build, lint, format or live command probe runs in this session.
  The graph CLI cannot start because an unverified generation is active;
  direct source reads supply the evidence and establish no graph coverage.
- Cancellation during capture preparation can precede the spawn without
  preventing it. Record that admission-to-spawn interval separately from the
  already-tested cancellation before `run_command` begins.
- The overflow fallback maps a negative integer beyond the float range to
  positive infinity. Record the lost deadline for an accepted numeric input.
- A ready pidfd suppresses owner lateness even after an overdue deadline.
  Record the misleading timing evidence independently of outcome precedence;
  the exit outcome itself remains correct.
- These findings meet the existing cancellation and truthful-record acceptance
  bar. Their triage belongs in one flat integration sibling immediately after
  this review, before the later implementation consumers. No new context node
  or second adversarial review is needed for this handoff.

## Findings

Coordinates refer to producer commit `b41c7492`, which is the parent of this
review. All three wrong results below are derived from source, not observed in
a fresh execution. The review makes no production or test-code changes.

### R1 — P1: a cancellation received during capture setup still starts the command

Code: `agents/linux/testanyware_agent/exec_owner.py:495-504` and `:521-554`;
`agents/linux/testanyware_agent/exec_registry.py:144-146`.

Input and ordering: admit an attached request such as
`{"id":"before-spawn","command":"touch /tmp/cancelled-command-ran; sleep 30","timeout":60}`.
Let the owner pass its cancellation check at line 495, then pause it during
capture-root or file preparation, before the `Popen` at line 528. Send
`/exec/cancel` for the admitted id and let it return `terminating`. Resume
the owner. This is an ordinary inter-thread ordering; a delayed filesystem
operation can widen the interval without any replacement execution backend.

Derived wrong result: the pipe holds the cancellation, but there is no further
check before `Popen`. The shell starts after cancellation and can perform its
side effect before the owner reaches the first poll. If it still runs, the
record says `cancelled` with `beforeStart: false`; if it has already exited,
the kernel-exit branch can return `exited`. Neither prevents the new work.
Shutdown uses the same signal and has the same interval for an admitted owner.
This contradicts the root requirement to stop new work on cancellation and
the producer's stated contract that cancellation after admission but before
spawn produces `beforeStart: true` without starting a process.

The control at `agents/linux/tests/test_exec_cancellation.py:242-253` requests
cancellation before entering `run_command`, so it exercises only the first
check. The tombstone control exercises an earlier admission path. Neither
guards cancellation that arrives after that check and before the spawn.

### R2 — P2: an accepted negative timeout beyond float range disables the deadline

Code: `agents/linux/testanyware_agent/system_endpoints.py:53-59`;
`agents/linux/testanyware_agent/exec_owner.py:72-77` and `:537-547`.

Input: an attached `/exec` whose command is `sleep 120` and whose `timeout`
is the JSON integer negative ten to the power 400 (a minus sign, `1`, then
400 zeros). This is a standard JSON number and is below Python's normal
integer-decoding digit limit. Validation accepts every integer other than a
boolean; it does not reject negative values.

Derived wrong result: converting that integer to float raises `OverflowError`.
The fallback returns positive infinity for either sign, bypassing the
`max(0.0, ...)` clamp. Consequently the deadline becomes infinite, and the
command runs until natural exit or explicit cancellation. Smaller negative
timeouts clamp to zero. A value that is even further below zero therefore
changes an immediate deadline into no deadline, with no HTTP validation error
or record explaining that change. This is part of the newly added saturation
path, not the repaired positive poll-range overflow.

The huge-timeout controls at
`agents/linux/tests/test_exec_cancellation.py:316-331` cover only positive
integers. The non-finite controls cover float literals, not signed integer
overflow, so they do not distinguish this failure.

### R3 — P2: pidfd readiness erases evidence of an overdue owner

Code: `agents/linux/testanyware_agent/exec_owner.py:210-216` and `:547-560`.

Input and ordering: use a one-second timeout and a leader that exits after
two seconds. Delay the owner after spawn and before its first kernel poll
until three seconds after spawn. The deadline and the leader exit are then
both observable when the owner resumes. This can arise from scheduling delay;
the existing leader-wait seam can also represent the ordering in a control.

Derived wrong result: the zero-time poll finds the pidfd readable and returns
True before checking whether its end time has passed. The owner correctly
chooses the kernel exit outcome, and its event list includes the one-second
deadline. Nevertheless line 560 forces `ownerLatenessMs` to zero because
`exited_first` is True. The record hides roughly two seconds of lateness,
including the breach of the specified 500 ms owner-lateness bound. This does
not call for changing the exit outcome: the Arbitration section expressly
allows an exit after the deadline to win, while requiring its owner lateness
to remain visible. The zeroing behaviour also existed before k220 and remains
in the arbitration code this producer extends.

`LostNotificationWaiter` in `agents/linux/tests/test_exec_lifecycle.py:413-425`
always returns False. `ExitBlindWaiter` in
`agents/linux/tests/test_exec_cancellation.py:174-215` does the same. Those
controls exercise the kernel-status priority but cannot expose the True
return that suppresses lateness here.

## Evidence and limits

- Inspected the producer's committed diff and current owner, registry,
  server, system handlers, accessibility lock and wait path, record models,
  cancellation, retention, responsiveness, shutdown, transfer and lifecycle
  controls, HTTP fixtures, protocol documentation and cited contracts.
- The registry serialises admission, publication, retention and cancellation.
  Its signal locks request and close together, writes once, and ignores requests
  after closure. The inspected before-start, tombstone and abandonment paths
  do not establish an additional write-to-reused-descriptor finding.
- The retained `old-fail-02/pytest.txt` shows the two expected old failures:
  health reaches its one-second client timeout and a procfs download sends
  1124 bytes beyond declared length zero. The earlier fixture-error attempt
  remains separate. These are producer evidence, not new runs.
- The retained mutant receipts show failures for removing the accessibility
  lock, putting health under it, reading downloads to EOF, skipping the kernel
  arbitration check, omitting shutdown cancellation, unbounded poll waits and
  cancelling on disconnect. The first download mutant's passing receipt is
  retained and disclosed by the producer; the replacement mutant fails the
  append and procfs controls. These mutations do not cover R1-R3's conditions.
- The download path uses one handle for `fstat` and bounded reads. Its
  truncation control checks a body shorter than the declared length; the
  production handler uses HTTP/1.0 framing and closes the connection. No
  additional extra-byte or wrong-version finding follows from those reads.
  In-place overwrite stays explicitly excluded by the agreed contract.
- The shutdown path closes admission, requests cancellation, closes the
  listening socket and waits for publication to a monotonic deadline. The
  unavailable-status fixture hides status past the cutoff. The review does
  not turn its inspected schedule or the producer's recorded green results
  into a fresh measured bound.
- Both `list_projects` and the attempted `check_index_coverage` failed before
  serving a query: `CBM CLI could not start because a pre-coordination or
  unverified CBM generation is active; close all CBM sessions and commands,
  then retry`. No graph generation, freshness or coverage was available.
  Conclusions are bounded to the directly inspected files and paths above.
- No tests, builds, lint, formatting or guest commands ran. The ten-clone
  acceptance trial remains separate; this review establishes no new guest
  reliability result.
