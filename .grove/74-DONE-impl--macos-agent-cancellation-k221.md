# macos-agent-cancellation-k221

## Goal

The macOS agent accepts explicit cancellation, retains finished records and
sends each download from one open file handle. It shuts down within bounds.

This leaf is the second part of candidate C. Leaf k219 owns the first part.

## Context

- Spec: sections "Agent shutdown", "Cancellation and CLI interruption",
  "Responsiveness", "Exec record", "Evidence transfer", "Bounds", "Test seams
  and required controls" and candidate C.
- ADR-0025 and ADR-0026.
- Old failing paths: `diagnostic:macos-disconnect-availability-host` and
  `diagnostic:download-consistency-local`.
- Retired leaves 40 to 42 and 50 to 52 hold the macOS causal evidence.

## Done when

- A health request during a command replies within 1 s.
- `/exec/cancel` meets the same contract as leaf k220. That covers the 5 s reply
  bound, idempotence, the exit-and-cancel race and the tombstone order.
- The agent retains each finished record for the retention time in the spec.
- A client disconnect does not cancel the command or change its record.
- A download reads one open file handle and sends exactly one file version.
- At SIGTERM, the agent meets the spec's "Agent shutdown" section, as in leaf
  k220. A VM stop does not replace this check.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference.
  - Corrected-pass: a health request during a command replies within 1 s.
    The old-fail case of this row is Linux.
  - Old-fail: `diagnostic:macos-disconnect-availability-host` fails with the
    shipped agent. Extended with a cancellation case, it shows that a
    disconnect changes nothing.
  - Old-fail: `diagnostic:download-consistency-local` fails with the shipped
    agent and shows one file version with the corrected agent.
  - New behaviour: `agent:macos:test` covers cancellation, idempotence, the
    exit-and-cancel race and the tombstone order.
  - New behaviour: `agent:macos:test` covers SIGTERM during a command. A
    second case holds a leader whose status stays unavailable.
- `agent:macos:test` passes.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The single-handle download does not make an in-place write readable as one
version. The spec excludes that case.

## Decisions (running log)

- D1. The cancellation signal lives in `ProcessRunner.swift`, because the k3
  and k12 detectors compile that file alone. It is a close-on-exec pipe. The
  kqueue waiter watches the read end beside the `NOTE_EXIT` source, so a
  cancellation wakes the owner at once. The signal closes at publication, and
  a later request writes nothing, as on Linux (k220).
- D2. `LeaderWaiter.waitExit` takes the signal as `wake`. The termination
  waits pass no signal and drop the read source. A readable pipe stays
  readable, so a kept source would make each `kevent` return at once.
- D3. The registry is a new file, `ExecRegistry.swift`, in the
  `TestAnywareAgent` library. It copies the Linux registry: 300 s, 64 entries,
  oldest first, no eviction of a running command. It stores the whole `/exec`
  reply, because the cancel reply's `record` is that reply.
- D4. Each blocking wait of the cancel endpoint runs on a new thread. A wait
  of up to 10 s on a cooperative pool thread could starve the other handlers.
- D5. A cancellation checks once more directly before `posix_spawn`, after the
  capture files exist (k236 R1). A tombstoned id gets `beforeStart: true` and
  lateness 0. A cancellation after admission and before the spawn gets
  `beforeStart: true` and its lateness from the request.
- D6. The download reads one handle through `SingleVersionFile`: `open`, then
  `fstat` for L, then `pread` up to L in 128 KiB chunks on the NIO thread
  pool. A short read throws in the body. Hummingbird 2.28.1 then closes the
  connection (source checked, cited at the call). The old `fileExists` check
  stays before the open. It keeps the old 404 replies, and it keeps the path
  lookup where the k7 diagnostic places its pause. The length never comes
  from it.
- D7. At SIGTERM the agent closes admission and cancels every running
  command. It triggers the Hummingbird graceful shutdown, which closes the
  listener. It waits for publication until 4.0 s after the signal. Then it exits with
  status 0. A second SIGTERM kills it at once. The handler calls
  `ExecRegistry.shutDown`, the routine that the unavailable-status control
  runs.
- D8. The binary takes an optional second argument, the listen address. Host
  tests pass 127.0.0.1. Without it the agent listens on 0.0.0.0, as before.
- D9. `AgentServer.swift` imports `NIOPosix` and `ServiceLifecycle` without
  new manifest entries. The file already imports `NIOCore` and `HTTPTypes` in
  the same way, and `Package.resolved` stays unchanged.
- D10. The transport controls start the agent binary on loopback and use raw
  HTTP. They can therefore close, reset or half-close a connection like the
  k13 cases. `TESTANYWARE_AGENT_UNDER_TEST` points them at another binary. The
  test target depends on the executable, so `swift test` builds it fresh.
- D11. The binary has no leader-wait seam. The unavailable-status shutdown
  control therefore runs `ExecRegistry.shutDown` in the test process with the
  blind waiter of k219. The binary SIGTERM control uses a cooperative
  command.
- D12. The k13 host diagnostic builds the checkout agent from source, so it
  cannot run against the shipped agent. The extended k13 control is
  `AgentProcessTests`: the close, reset and half-close cases, record
  retention and a cancellation after a close. The old-fail run points those
  tests at the shipped binary, as k220 ran its controls against the old Linux
  agent. The k13 tools stay unchanged.
- D13. The test validity checker had a k219 placeholder that refused every
  `cancelled` outcome. It now follows the spec: a cancelled command that
  started has a termination block and a leader, and one that never started
  has neither.
- D14. Eviction breaks a tie between equal times by insertion order. The
  first full `agent:macos:test` run failed
  `testARunningCommandIsNeverEvicted` twice. Two records published at one
  fake-clock time had equal ages, so dictionary order chose the evicted one.
  On that build, 4 of 8 repeat runs of the registry tests failed. After the
  change, 12 of 12 passed. That run's log was not kept in the evidence.
- D15. The component page conversion landed first as its own change,
  `d3773deb`. The content edit follows in the task commit, in both copies.
  `agent-protocol.md` now names macOS for `/exec/cancel`, the id registry,
  the before-start record, the SIGTERM exit and the download rule.
- D16. Every evidence run below is in
  `experiments/exec-reliability/evidence/macos-agent-cancellation-k221/`.
  `measure.sh` records the source, the dirty diff, the command, its output,
  its exit and its time. It also records the digests of every
  `agents/macos` file and the shipped agent before and after. Each run had
  equal digests.
- D17. The first measurements used a disconnect control that read the record
  after a marker file. `corrected-pass-01` passed 77 of 77, and
  `repeat-01` and `repeat-02` passed. `repeat-03` failed the half-close case
  with `terminating`. The command wrote the marker and still ran for a
  moment, and the read is a cancellation. The control now waits until the
  kernel no longer holds the leader, not even as a zombie. The owner reaps
  only after it chose the outcome. The old runs stay as history.
- D18. Final runs on the corrected control: `corrected-pass-02` passed
  `agent:macos:test` with 77 tests and no failure. `repeat-04` to
  `repeat-06` each passed the 61 tests of the new and owner classes.
- D19. Old-fail, `old-fail-02`: the transport controls ran against the
  shipped agent, SHA-256 `22dd56ce…a5311bab9f5`. Eight of ten failed. The
  disconnect control passed its first step: each command ran to its end
  after a close, a reset and a half-close. It failed at retention, because
  `/exec/cancel` answered 404. At SIGTERM the shipped agent still ran after
  10 s, and the child outlived it. Health during a command and the plain
  download passed, as expected for the old macOS agent. `old-fail-01` used
  the earlier disconnect control and failed the same eight. Process
  snapshots after both runs show no orphan.
- D20. Old-fail, `download-k7-01`: the unchanged task
  `diagnostic:download-consistency-local` exited 0, with a complete check.
  On the shipped agent, the rename case gave the failure-D bytes as a
  complete frame, and 2157 of 54580 stress reads did the same. On the
  checkout agent, both rename cases gave one whole version, and all 57995
  stress reads were whole versions. The in-place overwrite stayed torn on
  both agents, which the spec excludes. The interrupted Host CLI left its
  temporary file, which leaf k222 owns. This run used the test source before
  D17. The agent sources were the final ones.
- D21. Discrimination, `mutants/run-01`: five mutants of a scratch copy each
  failed their control. Three mutants are an owner that ignores a
  cancellation, arbitration without the kernel wait and a shutdown that
  cancels nothing. The other two are a download that opens the path again
  and eviction without the limit.
  The copy equalled the checkout before and after, and each file was
  restored. `ProcessRunner.swift` still compiles alone with the k12 probe,
  with and without `EXIT_TRACE`.
- D22. Limits. A stalled download read holds one NIO thread pool thread,
  as the old `FileIO` read did. Each waiting cancellation holds one thread.
  The binary SIGTERM control uses a cooperative command. The in-place
  overwrite stays excluded. No live guest ran in this leaf. The ten-clone
  trial of k225 stays the acceptance, and guest acceptance stays
  **0/10, unavailable**.
- D23. The first `measure.sh` recorded `jj diff --git` of the whole working
  copy, and that diff held the evidence directory too. Each run therefore
  copied the earlier records, and the last diff had 596 MB.
  `filter_dirty_diff.py` removed the sections under the evidence directory
  from each recorded diff. Each `.note` keeps the original size and SHA-256.
  The filtered diffs show two source states, before and after D17, as the run
  order predicts. `measure.sh` now leaves the directory out.
- D24. The review leaf is macos-agent-cancellation-k237, directly after this
  leaf and ahead of host-cli-control-k222. This session has no provider
  evidence, so the review carries no Creator line. No node closes, because
  the root still has live leaves.
