# linux-download-consistency-k14


## Goal
Establish the Linux download consistency evidence independently, reusing the
macOS inquiry's bounded file/HTTP/Host CLI diagnostics without inheriting its cause.


## Context
- Reuse the fixtures and report from `download-consistency-k7` and the controller
  and route table from `diagnostic-foundation-k11`.
- Linux `handle_download`/server `_handle_download`, Rust streaming/download
  transport fixtures, and ADR-0001. Inspect exact source before attribution.
- Failure D was measured on macOS only. A Linux defect or pass is new evidence.
## Done when
- Freeze a finite Linux matrix for different-length atomic replacement,
  in-place truncation/growth, immutable binary data and interrupted bodies.
  Run lower-level controls first; use at most one unchanged and one instrumented
  Linux clone if a guest route is needed. Respect the common evidence/bounds.
- Force suspected metadata/open/read interleavings at an existing file or HTTP
  seam. Record file handle/identity and metadata, advertised framing, emitted
  bytes, received bytes and host temporary-file/finalization result. A failed
  read stays failed even if another path yields usable evidence.
- Check complete allowed versions versus detected failure, premature EOF,
  immutable binary bytes (`00ff80410a`) and flipped/truncated independent copies.
  Recheck originals afterward. Preserve fixed-memory streaming and <=150 s
  transfer bounds. Do not import lifecycle-record corruption checks here.
- Reuse any small immutable versioned-publication control from k7 as an
  evidenced option; state tested/unavailable and its completeness oracle.
  Selecting the production consistency contract belongs to k8, and the selected
  contract still needs implementation and acceptance-preparation verification.
- Deliver verified Taskfile diagnostics and
  `docs/experiments/exec-reliability-linux-download-consistency.md` with per-case
  evidence, old-path failure or explicit non-reproduction, differences from
  macOS, design implications and complete resource audit.
## Notes
Do not change production framing/finalization to match a presumed macOS cause.
If the reusable fixtures are insufficient, narrow the Linux question or
decompose it; do not reopen or absorb the macOS inquiry.
