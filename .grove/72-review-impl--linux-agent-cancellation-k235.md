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
