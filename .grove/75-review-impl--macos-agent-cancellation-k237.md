# macos-agent-cancellation-k237

**Reviews:** macos-agent-cancellation-k221

## Goal

Try to break the macOS agent change of macos-agent-cancellation-k221. The
Host CLI leaf host-cli-control-k222 builds on its wire shapes next. Find each
cancellation, exit or shutdown order that gives a record the kernel did not
produce. Find each request that a running command or a waiting cancellation
blocks. Find each download that sends a byte of another file version or a
byte past its declared length. Find each download that reports success for
wrong bytes.

## Context

- Spec `docs/specs/guest-command-lifecycle.md`: sections "Arbitration",
  "Termination after a timeout or a cancellation", "Agent shutdown",
  "Cancellation and CLI interruption", "Responsiveness", "Exec record",
  "Evidence transfer", "Bounds" and "Test seams and required controls".
- ADR-0024, ADR-0025 and ADR-0026.
- The contract of k221 is its `Done when` list and its notes.
- The code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`
  (the cancellation signal and the owner), `ExecRegistry.swift`,
  `SingleVersionFile.swift`, `ExecRecordMapping.swift`, and
  `agents/macos/Sources/testanyware-agent/AgentServer.swift` and `main.swift`
  (the cancel endpoint, the download and the SIGTERM path).
- The controls: `agents/macos/Tests/TestAnywareAgentTests/AgentProcessTests.swift`,
  `ExecCancellationTests.swift`, `ExecRegistryTests.swift`,
  `SingleVersionFileTests.swift` and the changed `ExecRecordValidity.swift`.
  `task agent:macos:test` runs them.
- Retained evidence: `experiments/exec-reliability/evidence/macos-agent-cancellation-k221`.
- Documents: `docs/architecture/agent-protocol.md` and
  `docs/components/agents-macos.md`.
- The Linux counterpart and its review: linux-agent-cancellation-k220,
  linux-agent-cancellation-k235 and linux-agent-cancellation-k236.

## Specific doubts

- One owner must handle the leader exit, the deadline and a cancellation in
  order. Check that a cancellation never wins over a leader that has exited.
  Check that a timeout never becomes a cancellation, and the reverse. Check
  the lateness of each path.
- The cancellation reaches the owner through a pipe that the kqueue waiter
  watches. Compare every path that closes the pipe with every path that
  writes to it or registers it. Check the termination waits, which drop the
  read source, and the fallback status poll without a source.
- Each waiting cancellation runs on a new thread. Check whether many waiting
  cancellations can delay health or another request.
- Retention: 300 s, 64 at most, the oldest first, never a running command.
  Check eviction, tombstones and duplicate ids across every registry entry
  point. Check the publish precondition.
- Shutdown at SIGTERM: check the dispatch signal source, the order of
  admission, cancellation and listener close, and the bound on the whole
  exit. The binary has no leader-wait seam, so the unavailable-status case
  runs `ExecRegistry.shutDown` in the test process. Judge whether that
  control covers the binary.
- Download: one handle, exactly the declared length, and a closed
  connection on a short read through Hummingbird 2.28.1. The `fileExists`
  check before the open stays. Check that the length never depends on it.
- The old-fail controls ran the transport tests against the shipped agent
  instead of the k13 host diagnostic. Judge whether that run shows the
  stated old failure. Every control must be able to fail on the new code.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings, not fixes.

## Notes

Review the producer commit of macos-agent-cancellation-k221 against the
current source. Do not replace a golden, and do not change the global CLI or
app.
