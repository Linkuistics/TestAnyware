# macos-agent-lifecycle-k219

## Goal

The macOS agent owns the lifecycle of each guest command through
`posix_spawn` and kqueue. It returns the typed exec record.

This leaf is the first part of candidate C. Leaf k221 owns the rest of C.

## Context

- Spec: sections "Exec record", "Bounds", "Compatibility", "Test seams and
  required controls" and candidate C.
- ADR-0023 and ADR-0024.
- Agent code: `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift` and
  `agents/macos/Sources/testanyware-agent/AgentServer.swift`.
- Old failing paths: `diagnostic:macos-completion-old-path` and
  `diagnostic:macos-exec-outcomes-old-path`.
- Retired leaves 17 to 39 hold the macOS causal evidence.

## Done when

- The exec types move into the vendored `TestAnywareAgentProtocol` module. An
  encoding drift test guards them.
- The owner meets the same lifecycle rules as leaf k217. That covers the group,
  the held leader, the grace, the kill-and-reap bound, the tombstone order,
  the identity checks and the capture deadline.
- The exec reply carries the typed record. The legacy fields keep the macOS
  values of the old agent.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference.
  - Old-fail: a child that ignores SIGTERM receives SIGKILL after the grace.
    The old path is the term-ignored case of
    `diagnostic:macos-exec-outcomes-old-path`.
  - New behaviour: the leader status is unavailable at K+1.4 s.
  - New behaviour: the capture is large or stalls.
  - Old-fail: `diagnostic:macos-exec-outcomes` shows the exact bytes, exit
    23, signal death and the spawn errno. The old path is
    `diagnostic:macos-exec-outcomes-old-path`.
  - Old-fail: `diagnostic:macos-completion-old-path` at `free-1` fails on the
    old runner. The same task passes with `EXPECT=clean` on the new runner.
  - Compatibility: the legacy projection equals the recorded replies of
    today's macOS agent.
- `agent:macos:test` passes 20 consecutive times.
- Add the new macOS replies to the reader tests of leaf k218.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The cause of the original macOS post-timeout stall is unresolved. No part of
this leaf may depend on it. Keep the retained samples, and report a
recurrence as a failure. Cancellation and the single-handle download belong to
leaf k221.
