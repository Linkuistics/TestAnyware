# linux-agent-lifecycle-k217

## Goal

The Linux agent owns the lifecycle of each guest command and returns the typed
exec record. A Linux agent test task runs the controls that prove it.

This leaf is the first part of candidate B. Leaf k220 owns the rest of B.

## Context

- Spec: `docs/specs/guest-command-lifecycle.md`, sections "Exec record",
  "Bounds", "Test seams and required controls" and candidate B.
- ADR-0023 (typed record) and ADR-0024 (one owner ends each command).
- Agent code: `agents/linux/testanyware_agent/system_endpoints.py`,
  `server.py` and `models.py`. Tests: `agents/linux/tests/`.
- Old failing path: `diagnostic:linux-timeout-lifecycle-old-path` and
  `experiments/exec-reliability/linux_k5*`.
- Retired leaves 43 to 46 (Linux timeout lifecycle) hold the causal evidence.

## Done when

- Add the Taskfile task `agent:linux:test`. It runs the Linux agent tests in
  a locked Python project. Name it after `agent:macos:test`.
- The owner starts each command in its own process group and holds the
  leader. It applies the deadline, the 2 s grace, SIGKILL and the 2 s
  kill-and-reap bound in the spec's tombstone order.
- The owner checks the identity of each known child. The capture read has a
  deadline. A failed inspection never proves absence.
- The exec reply carries the typed record. It holds the outcome, the lifecycle
  times and the termination record under the spec's validity rules.
- `models.py` holds the exec record model.
- The legacy fields keep the Linux values of the old agent. An old Host CLI
  prints the same output and returns the same exit code.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference. Each
  control runs in `agent:linux:test`:
  - Old-fail: the group ends after a timeout. The old path is
    `diagnostic:linux-timeout-lifecycle-old-path`.
  - Corrected-pass: a child that ignores SIGTERM receives SIGKILL after the
    grace. The old-fail case of this row is macOS. Record any old Linux
    failure as evidence.
  - New behaviour: the leader status is unavailable at K+1.4 s.
  - New behaviour: the capture is large or stalls.
  - Corrected-pass: the exact bytes, exit 23, signal death and the spawn
    errno survive. This is the Linux equivalent of
    `diagnostic:macos-exec-outcomes`.
  - Compatibility: the legacy projection equals the recorded replies of
    today's Linux agent.
- Record the new Linux exec replies as fixtures for leaf k218.
- The existing Linux agent tests and `cli:test` pass.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

Cancellation, the threaded server, record retention and the exact-length
download belong to leaf k220. Do not start them here. Do not replace a golden.
New goldens use `TESTANYWARE_AGENT_BIN_OVERRIDE`.
