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

## Decisions (running log)

- The lifecycle controls need a Linux kernel, because the owner uses pidfd and
  `waitid`. `agent:linux:test` runs the suite on the host first. Then it runs
  the suite in the pinned k5 image, with no network and uid 1000. In the
  container, `TESTANYWARE_LINUX_CONTROLS=required` turns a skipped control into
  a failure. The container borrows pytest from the locked project.
- Every record key is always present. A key that does not apply holds `null`.
  The spec example shows `"termination": null`, so "present" means not null.
- The unavailable-status control replaces only the leader waiter. The stalled
  capture control replaces only the capture `open`. Both seams sit in
  `OwnerSeams`. The leader and the files stay real.
- The spawn-failure cases run in a fresh interpreter under `RLIMIT_NPROC` 1,
  as k5 did. A fork of the threaded test process could deadlock.
- An attached request with a command that is not a string, a timeout that is
  not a number, or an invalid `id` now returns 400 `exec_failed`. The old
  agent returned 200 with -1 and a Python error text. The Host CLI types
  `command` as a string and `timeout` as an integer, so it never sends these.
- The k5 matrix ran once against the new agent. The old-path detector fails
  with seven missing defects, and each receipt equals `run-03`. The report
  section "Run against the lifecycle owner (leaf k217)" records it.
- No old Linux failure exists for the SIGTERM-ignoring row. In k5 `run-03`,
  `deadline-term-ignored` shows no survivor, because the old agent sent
  SIGKILL to the leader at the deadline.
- The fixtures for leaf k218 are in `agents/linux/tests/fixtures/exec-records`.
  `task agent:linux:record-exec-replies` records them again. Times, PIDs and
  generated ids differ from run to run.
- `ruff format` ran only on the new files. Eight old files do not match the
  formatter. Formatting them is not part of this leaf.
