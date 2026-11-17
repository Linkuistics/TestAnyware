# download-consistency-k7


## Goal

Locate the boundary that labels a truncated mutable-file download successful,
and establish macOS evidence for supported consistency choices. Deliver bounded
file/HTTP/Host CLI diagnostics that the separate Linux comparison can reuse.



## Context

- Issue #14 failure D; preserve downstream `macos-candidate-02/` artifacts
  `after-abrupt-03-state-002.json`, `after-abrupt-03-collect-002.json`,
  `after-abrupt-03-initial.json` and `after-abrupt-03-launch.json` together.
- macOS `handleDownload` and exact resolved Hummingbird `FileIO.loadFile`
  dependency source. The Linux comparison belongs to
  `linux-download-consistency-k14`, which consumes this leaf's fixtures/report.
- Rust `AgentClient.open_download`/`download`, streaming helpers and existing
  `endpoint_integration`, `streaming_tests`, `transport_seam`; ADR-0001.

## Done when

- Hash/retain original 25-byte pending record, malformed terminal prefix and
  successful receipt. State their evidence without presupposing which layer
  supplied the wrong length or file identity.
- Freeze bounded agent/HTTP/Host CLI cases for different-length atomic
  replacement, in-place truncation/growth, immutable binary data and interrupted
  bodies. Use lower-level controls first and at most one unchanged and one
  instrumented macOS clone if a guest route is needed. Reuse k11's controller
  and route table; an unavailable route stays explicit. No Linux workloads
  belong to this leaf.
- Force suspected metadata/open/read interleavings at the existing file or
  transport seam. Record handle/inode or equivalent identity, metadata,
  advertised framing, emitted/received bytes and host temporary-file/finalization
  behavior. Inspect exact dependency source before assigning blame. Preserve
  streaming/bounded-memory constraints, <=150 s transfer bounds and raw errors.
- Establish whether a result is a complete allowed version or detected failure.
  Verify immutable binary reads, premature EOF and failure cleanup/finalization.
  Independently exercise immutable versioned publication/read with a completeness
  oracle as a small control if it fits this inquiry; otherwise name the precise
  missing experiment before design relies on that option. Label it tested or
  unavailable rather than an already selected fix. The selected contract still
  needs later implementation and acceptance-preparation verification.
- Verify transfer bytes/framing with the immutable binary oracle `00ff80410a`
  and flipped/truncated independent copies; recheck originals afterward. Never
  normalize or retry malformed evidence into success. Lifecycle-record checks
  (missing outcome and contradictory termination records) and the complete four
  corruption controls belong to preparation after reviewed design/implementation.
- Deliver verified Taskfile cases and
  `docs/experiments/exec-reliability-download-consistency.md` with controlled
  old-path failure or explicit uncertainty, responsible-boundary evidence,
  tested alternatives/limits, retained bytes and complete owned-resource audit.

## Notes

Atomic guest rename alone does not prove remote-read consistency. A limited
mutable-read contract plus a stable alternative is permitted; design chooses
between evidenced options. Do not patch FileIO or host finalization as a
presumed repair. Decompose a dependency/race inquiry that exceeds this session.
This leaf's raw malformed JSON is historical evidence, not a future lifecycle
schema to adopt. Keep the Linux comparison independently usable in k14.

## Decisions (running log)

- The host route replaced the guest route. The macOS agent binary takes a port argument, and the Host CLI takes `--agent HOST:PORT`. The shim rewrites the wildcard bind to loopback, and the controller refuses an agent that listens elsewhere. No clone was needed, because the host route reproduced the failure D digest exactly.
- The seam is a `DYLD_INSERT_LIBRARIES` shim, not a patch of `FileIO`. It traces and pauses calls by path below the case directory only. The source of the agent and its dependencies stays unchanged.
- A calibration download picks the paused lookup, because the shipped agent calls `lstat` and the checkout agent calls `stat` for the size. The dependency pins of the shipped agent are unknown, because `v2.8.0` has no `Package.resolved` for the macOS agent.
- The responsible boundary is `FileIO.loadFile` and `FileIO.readFile` in Hummingbird 2.28.1. The size comes from a path lookup, and a later path open reads the file. Nothing checks that both found the same inode. The trace shows two inodes in one response.
- The stress cases ran with no pause, to show that the shim does not create the race. Both agents returned the failure D digest with a complete frame, 1,887 and 2,794 times.
- Stable publication to a new name once is tested as a control on the host only. It is not a selected repair. The guest and relay versions of that test are not run.
- The Host CLI leaves its temporary file after SIGTERM during the body. The report records this for `lifecycle-evidence-k8`. This leaf does not repair it.
- The checkout agent builds in a temporary directory outside the repository. A development run had its 854 MB build tree in its outer directory, so that run was removed. The recorded run is `local-01`, with no retry.
- The review leaf `download-consistency-k209` comes before `linux-download-consistency-k14`. That order lets the review finish before k14 reuses the tooling.
- Verification: `diagnostic:download-consistency-tools` passes, with 16 tests. `local-01` exited 0, its check is complete, and a replay of the check gave an equal result.
