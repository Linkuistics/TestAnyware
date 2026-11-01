# download-consistency-k7


## Goal

Locate the boundary that labels a truncated mutable-file download successful,
and establish evidence for supported consistency choices.



## Context

- Issue #14 failure D; preserve downstream `macos-candidate-02/` artifacts
  `after-abrupt-03-state-002.json`, `after-abrupt-03-collect-002.json`,
  `after-abrupt-03-initial.json` and `after-abrupt-03-launch.json` together.
- macOS `handleDownload` and exact resolved Hummingbird `FileIO.loadFile`
  dependency source; Linux `handle_download` and server `_handle_download`.
- Rust `AgentClient.open_download`/`download`, streaming helpers and existing
  `endpoint_integration`, `streaming_tests`, `transport_seam`; ADR-0001.

## Done when

- Hash/retain original 25-byte pending record, malformed terminal prefix and
  successful receipt. State their evidence without presupposing which layer
  supplied the wrong length or file identity.
- Freeze bounded agent/HTTP/Host CLI cases for different-length atomic
  replacement, in-place truncation/growth, immutable binary data and interrupted
  bodies. Compare macOS and Linux using at most one unchanged and one instrumented
  clone per OS; a Linux result is new evidence, not a historical macOS diagnosis.
- Force suspected metadata/open/read interleavings at the existing file or
  transport seam. Record handle/inode or equivalent identity, metadata,
  advertised framing, emitted/received bytes and host temporary-file/finalization
  behavior. Inspect exact dependency source before assigning blame. Preserve
  streaming/bounded-memory constraints, <=150 s transfer bounds and raw errors.
- Establish whether a result is a complete allowed version or detected failure.
  Verify immutable binary reads, premature EOF and failure cleanup/finalization.
  Independently exercise immutable versioned publication/read with a completeness
  oracle; label it tested or unavailable rather than an already selected fix.
- Verify the root's exact byte/corruption controls: appended stdout LF, flipped
  binary byte, missing terminal outcome and contradictory termination records,
  each in an independent copy; recheck originals afterward. Never normalize or
  retry malformed evidence into success.
- Deliver verified Taskfile cases and
  `docs/experiments/exec-reliability-download-consistency.md` with controlled
  old-path failure or explicit uncertainty, responsible-boundary evidence,
  tested alternatives/limits, retained bytes and complete owned-resource audit.

## Notes

Atomic guest rename alone does not prove remote-read consistency. A limited
mutable-read contract plus a stable alternative is permitted; design chooses
between evidenced options. Do not patch FileIO or host finalization as a
presumed repair. Decompose a dependency/race inquiry that exceeds this session.
