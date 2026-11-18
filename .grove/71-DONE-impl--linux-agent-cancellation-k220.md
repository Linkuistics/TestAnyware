# linux-agent-cancellation-k220

## Goal

The Linux agent stays responsive while a command runs. It accepts explicit
cancellation, retains finished records and sends exact-length downloads. It
shuts down within bounds, and its accessibility endpoints stay serialised.

This leaf is the second part of candidate B. Leaf k217 owns the first part.

## Context

- Spec: sections "Agent shutdown", "Cancellation and CLI interruption",
  "Responsiveness", "Exec record", "Evidence transfer", "Bounds", "Test seams
  and required controls" and candidate B.
- ADR-0025 (cancellation is explicit and disconnect is not cancellation) and
  ADR-0026 (a download sends one open file version).
- Old failing paths: `diagnostic:linux-disconnect-container` and
  `diagnostic:linux-download-consistency-container`.
- Retired leaves 47 to 49 and 53 to 55 hold the Linux causal evidence.

## Done when

- The agent server serves each request on its own thread. A health request
  during a command replies within 1 s.
- All accessibility endpoints share one lock, because AT-SPI is not safe
  across threads. The exec, cancel, health, upload and download handlers never
  take that lock.
- `/exec/cancel` ends the command through the lifecycle owner of leaf k217. Its
  reply waits at most 5 s (`waitMs` 5000).
- Cancellation is idempotent. The exit-and-cancel race resolves by the spec's
  arbitration rule. The tombstone order holds.
- The agent retains each finished record for the retention time in the spec.
- A client disconnect does not cancel the command or change its record.
- A download sends exactly the declared length of one open file version.
- At SIGTERM, the agent meets the spec's "Agent shutdown" section. It stops
  admission, cancels every running command and waits only for bounded record
  publication. A leader whose status stays unavailable does not delay the
  exit. A VM stop does not replace this check.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference. Each
  control runs in `agent:linux:test`:
  - Old-fail: the agent stays responsive during a command. The old path is
    `diagnostic:linux-disconnect-container`.
  - Corrected-pass: a health request and a cancellation reply within 1 s
    while another request holds the accessibility lock. Two concurrent
    accessibility requests never run inside AT-SPI together.
  - Corrected-pass: a disconnect changes nothing, and the agent retains the
    record. The old-fail case of this row is macOS.
  - New behaviour: cancellation, idempotence, the exit-and-cancel race and
    the tombstone order hold.
  - New behaviour: at SIGTERM during a command, the group ends and the agent
    exits within the spec's bounds. A second case holds a leader whose status
    stays unavailable.
  - Old-fail: a download sends no extra bytes. The old path is
    `diagnostic:linux-download-consistency-container`.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The cancel types in the Rust protocol crate belong to leaf k222. Test the
endpoint here through raw HTTP in the agent tests. Do not replace a golden.

Review integration macos-agent-lifecycle-k234 found a Linux form of the macOS
finding R2. The request accepts any number as `timeout`. In
`LeaderWaiter.wait_exit`, `select.poll` raises `OverflowError` for a wait of
about 2147484 s or more. The host Python 3.12 showed this error. The source
shows the rest: the error comes after the spawn, the server replies HTTP 500,
and no owner ends the group.

The macOS owner now saturates its deadline and
waits at most 60 s in each kernel call. Bound each Linux poll the same way,
and add a control with a timeout of 20000000000 s.

## Decisions (running log)

- The registry of running commands, retained records and tombstones is a new
  module, `exec_registry.py`. The owner in `exec_owner.py` gets one more
  input, the cancellation signal of its command. The registry and the owner
  stay separate: the registry knows ids and retention, and the owner knows
  processes.
- The owner waits on the leader pidfd and on a wake pipe together, in one
  `poll`. A cancellation writes one byte to that pipe. The pipe closes when
  the record is published, and a closed signal ignores a later cancellation,
  so no write reaches a reused descriptor number.
- Each `poll` waits 60 s at most, as on macOS (k234 D3). A timeout that does
  not fit a float saturates to an infinite deadline. A NaN or infinite
  `timeout` gets 400 `exec_failed`, because the record cannot carry it as
  JSON. Standard JSON has no such literal, and the Host CLI sends an integer.
- Health never takes the AT-SPI lock and never calls AT-SPI. One probe thread
  calls `is_accessible()` under the lock. Health asks it for a fresh value and
  waits 0.5 s at most. Then health replies with the newest value that it has.
  Before the first probe finishes, that value is false. The spec says health
  never takes the lock. It does not say how health learns `accessible`.
- `/wait` takes the AT-SPI lock for each poll of the windows, not for its whole
  timeout. Each other accessibility endpoint holds the lock for its whole call.
- A repeated cancellation of a tombstoned id returns `unknown_id` again. The
  tombstone keeps its first time. No command ran under that id.
- A cancellation that arrives after admission and before the spawn gives
  `cancelled` with `beforeStart: true` and no spawn. A tombstoned `/exec`
  gives the same record with `ownerLatenessMs` 0, because the owner never held
  a pending event.
- The capture directory goes at publication, as section "Capture" orders. The
  retained record holds its bytes in memory. Section "Retention" says that the
  directory goes together with the record. The record no longer needs the
  directory, so the earlier removal loses nothing.
- At SIGTERM the agent closes admission, cancels every running command, stops
  the accept loop and waits for publication until 4.0 s after the signal. The
  owner schedule needs 3.8 s after its outcome, and 4.0 s is the sum of the
  two root bounds. Then the agent exits with status 0. A second SIGTERM during
  that wait kills the agent at once.
- A rejected `/exec` during shutdown gets 503 `exec_failed`. A malformed
  cancellation gets 400 `exec_failed`, like a malformed `/exec`. The spec names
  no token for either case. The duplicate id gets 409 `exec_id_in_use`, as the
  spec orders. The Host CLI maps that token in leaf k222. Until then it
  reports `AGENT_ERROR_UNKNOWN` with the wire string.
- Old-fail evidence, before any agent change: the agent package equalled
  base commit 3823eaa1b3, and the dirty diff held only tests and this file.
  The two old-fail controls of `tests/test_agent_process.py` ran in the
  pinned container against the old agent. Both failed as expected. Health
  raised `TimeoutError` at its 1 s client deadline while a command ran. The
  `/proc/self/status` download sent 1124 bytes past its declared length 0.
  The test and agent digests were equal before and after the run. An earlier
  attempt errored in a test fixture before any control ran, so it measured
  nothing. Both receipts are in
  `experiments/exec-reliability/evidence/linux-agent-cancellation-k220/`.
- The download control uses a procfs file. `fstat` gives size 0 and a read
  still gives bytes, so the control reproduces growth past L without a race
  or a seam.
- `task agent:linux:test` on 2026-11-18 after the change: ruff passed. The
  host run gave 50 passed and 71 skipped, because the host is darwin. The
  pinned container, with `TESTANYWARE_LINUX_CONTROLS=required`, gave 121
  passed in 68.99 s.
- Discrimination: each mutant below ran in the pinned container against a
  scratch copy of the agent, and each failed its controls. The router without
  the AT-SPI lock failed 2 responsiveness controls. Health under the lock
  failed 2. A download that reads to end-of-file failed the append and procfs
  controls. A cancellation that skips the kernel wait failed the race control.
  A shutdown that cancels nothing failed both SIGTERM controls, because the
  child outlived the agent. One `poll` for the whole wait failed 3 huge-timeout
  controls. A cancellation at 0.2 s, in place of disconnect handling, failed
  the 3 disconnect controls. A first download mutant kept the length bound and
  therefore did not recreate the old path. It passed, and its receipt is kept.
  The scripts and receipts are in the evidence directory, under `mutants/`.
- The old-path diagnostics `diagnostic:linux-disconnect-container` and
  `diagnostic:linux-download-consistency-container` stay unchanged. They did
  not run against the new agent. The two old-fail controls in
  `agent:linux:test` ran against the old agent instead, and they need no
  container image beyond the pinned one.
- `docs/architecture/agent-protocol.md` now states `/exec/cancel`, the
  duplicate id, the shutdown reply and the download rule. Its website copy
  was already behind the docs copy, for example on file transfer, so this
  leaf did not change it. Both copies of the Linux component page changed
  together. A conversion to controlled English landed first, as its own
  change.
- The k217 seam waiters in `test_exec_lifecycle.py` take the new `wake`
  argument. Their process helpers moved to `tests/processes.py`.
- After the final change, `task agent:linux:test` passed again: 50 passed and
  71 skipped on the host, 121 passed in the container. The five files of new
  controls then ran three more times in the container. Each run gave 60
  passed, so no timed control failed in four runs.
