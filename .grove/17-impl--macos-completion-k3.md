# macos-completion-k3


## Goal

Locate the boundary behind ordinary macOS synchronous completion timeouts.
Deliver a bounded diagnostic, retained causal evidence and a deterministic
regression candidate where the responsible ordering can be established.



## Context

- Issue #14 failure A, retained `macos-candidate-01/ordinary-08.json`, adjacent
  raw streams and subsequent failures in the downstream evidence.
- `ProcessRunner.runShell`/`waitForExit` and macOS `handleExec`; Rust
  `AgentClient.exec`, `endpoint_integration` and `transport_seam`.
- `ProcessRunnerTests.swift` has broad `pkill` cleanup. Replace it with
  positively identified owned fixtures before invoking `agent:macos:test`.
- The root brief supplies the common evidence, isolation and resource contract.
- Reuse `diagnostic-foundation-k11`'s controller, ledger and route table;
  report unavailable instrumented conditions explicitly rather than building
  a second shared harness in this inquiry.

## Done when

- Preserve/hash the original failure and identity receipts. Establish the old
  installed agent's source/build identity if possible; otherwise label current
  source builds as new conditions and keep the old provenance gap explicit.
- Freeze one fresh unchanged macOS clone's ten consecutive exact
  `printf ordinary-ok` commands, guest timeout 30 s and independent host bound
  50 s. Record every sample, including operation 8; do not replace failures.
- Differentiate child exit, wait registration/completion, capture read, handler
  response, client deadline and app relay using existing process/HTTP/transport
  seams. Compare serial execution with a small predeclared concurrent-request
  control against the same build. Unobserved phases remain unknown.
- If needed, use at most one isolated instrumented clone to force the suspected
  ordering at the highest existing seam. Retain evidence for and against each
  cause and an old-path defect detector. A clean run after non-reproduction is
  not repair, and captured stdout alone never locates the stalled phase.
- Verify ownership-scoped process-test cleanup, including an unrelated child
  left alive. Add a reusable bounded Taskfile diagnostic and instrument
  self-checks; keep ordinary checks green and known-failure probes explicit.
- Publish `docs/experiments/exec-reliability-macos-completion.md` with raw
  evidence references, item results, regression invocation, causal limits,
  design implications and complete owned-resource audit.

## Notes

Investigate ordinary completion before deliberate interruption. The historical
post-timeout case belongs to `macos-timeout-inspection-k4`. File-backed capture
already exists; capture-pipe EOF is not an established cause. Select no new
production wait/cancellation semantics here. Decompose if necessary and finish
only the first child; never absorb a sibling inquiry into this session.
