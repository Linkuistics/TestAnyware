# macos-agent-lifecycle-k233

**Reviews:** macos-agent-lifecycle-k219

## Goal

Try to break the macOS lifecycle owner of macos-agent-lifecycle-k219 before
the cancellation leaf macos-agent-cancellation-k221 builds on it. Find each
command whose record does not match what the kernel did. Find each case where
the owner can signal a process outside its group. Find each value of today's
macOS reply that changed for a Host CLI that reads only the legacy fields.

## Context

- Spec `docs/specs/guest-command-lifecycle.md`: sections "Guest command
  lifecycle", "Exec record", "Compatibility and version skew", "Bounds" and
  "Test seams and required controls".
- ADR-0023 and ADR-0024.
- The owner: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`.
- The wire types: `agents/macos/Sources/TestAnywareAgentProtocol/ExecRecord.swift`.
- The mapper: `agents/macos/Sources/TestAnywareAgent/ExecRecordMapping.swift`.
- The server: `handleExec` in `agents/macos/Sources/testanyware-agent/AgentServer.swift`.
- Recorded replies: `agents/macos/Tests/fixtures/exec-records`.
- Controls: `ExecOwnerTests.swift`, `ExecReplyFixtureTests.swift`, the k12
  record judge in `experiments/exec-reliability/outcomes_k12_check.py`, and
  the evidence in `experiments/exec-reliability/evidence/macos-agent-lifecycle-k219`.

## Specific doubts

- The owner reaps the leader, and only the owner may reap it. Check every
  path that ends without a reap, and the late-reap thread.
- The schedule: SIGTERM at T0, SIGKILL by T0 + 1.9 s, the status wait to
  K + 1.4 s and finalisation by K + 1.9 s. Check that no wait can block past
  its bound, including a kqueue error and a stalled capture read.
- The arbitration at the deadline: a leader that exits at the deadline must
  never give `timed_out`.
- The capture worker owns the read descriptors after the deadline. Check that
  an abandoned worker cannot read a reused descriptor or change a record.
- The spawn: `POSIX_SPAWN_CLOEXEC_DEFAULT`, the signal reset and the
  environment. Check what a command inherits from the agent.
- The legacy projection must equal today's macOS values for each outcome, and
  the record must pass every validity rule of the spec.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings, not fixes.
