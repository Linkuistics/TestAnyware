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

## Decisions (running log)
- Source: the Linux handler opens the path once (`system_endpoints.handle_download`),
  takes the length by `os.fstat` on that descriptor, then reads 1 MiB chunks from the
  same descriptor (`server._handle_download`). The shipped bundle `server.py` and
  `system_endpoints.py` equal the checkout. The macOS cause, a second path lookup, has
  no Linux counterpart in source. The cases test the descriptor seams instead.
- Fixture: one fresh container of the pinned k5 image
  (`python@sha256:dddfd7e0…`, arm64), uid 1000, no network except published loopback
  ports. Case files live on the container file system, not on a bind mount, so Linux
  file semantics apply. The Host CLI 2.8.0 reaches the agent through the Docker port.
  A probe on 2026-11-18 downloaded the oracle bytes this way.
- Seam: an in-container supervisor runs the unchanged handler class in a subclass
  that only records writes. It wraps `os.fstat` and the opened file at module
  boundaries. An armed hook changes the file inside the agent process at the
  descriptor seam, or pauses until the host resumes it. The writer process differs
  from production, but rename, truncate and write semantics do not depend on the
  writing process.
- Unchanged lane: the stress runs and the immutable publication control use
  the unchanged `run_server`, with no hooks.
- No clone: the question is the agent file and HTTP boundary, which the container
  reaches on a Linux kernel. The guest relay stays untested, as in k7. The report
  states this limit.
- 2026-11-18. Trials 1 and 2 (scratchpad, not retained) corrected the seam before any recorded run. The writer wrapper needed `closed` and `close`, because `StreamRequestHandler.finish` uses both. A killed agent left its `paused` marker, so `prepare` now removes stale markers and reports them. A restarted agent continues the request numbering, so `emitted-N.bin` names stay unique. The abandon client did not send its request.
- 2026-11-18. The checker is independent of the controller: its own oracle, its own retained-record digest and its own case list. It requires each armed hook to run at its event, and the agent's socket bytes to equal the raw client's bytes. A 64 MiB transfer must stay within 150 s with a peak RSS rise of 16 MiB at most.
- 2026-11-18. Tasks `diagnostic:linux-download-consistency-{container,check,tools}` were verified with forced failures. An unused import fails `tools`. A removed record and an extended `received.bin` fail `check`. An existing OUT is refused with exit 2. The first refusal check was void: the runner lacked its execute bit and failed with 127. It was repeated after `chmod +x`.
- 2026-11-18. The evidence keeps bodies up to 8 MiB as files. The 64 MiB body is kept by digest only. A run is about 137 MB, in line with other retained runs here.
- 2026-11-18. Recorded run `local-01` passed every gate. Replacement by rename gave the old version in all four cases, so the k7 macOS failure is not reproduced on Linux. Growth after `fstat` breaks framing, and an in-place overwrite between reads gives a torn body that both clients accept. The report is `docs/experiments/exec-reliability-linux-download-consistency.md`.
- 2026-11-18. The report is design input for `lifecycle-evidence-k8`, so it earns an independent read. Review leaf `linux-download-consistency-k211` was inserted ahead of that design leaf.
- 2026-11-18. After the run, the checker took a relative RESULT as relative to the working directory, not to OUT. A forced-failure trial had left a stray `check.json` at the repository root. The checker now joins RESULT to OUT, as in k7, and the stray file was removed. The recorded check used the default path. A re-judge into `local-01/rejudge-k14.json` gave the same verdict.
