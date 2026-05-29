# Streaming file transfer for the agent protocol

/ Status: accepted /

## Context

`/upload` and `/download` carried the entire file base64-encoded inside a JSON
body (`{ "path": ..., "content": "<base64>" }`). This had three problems: (1)
base64 inflates payload by 4/3 plus encode/decode CPU; (2) both the agent and
the CLI client buffered the whole file in memory; (3) the effective size cap
differed silently per agent because each HTTP stack imposed its own request-body
limit — macOS/Hummingbird capped files at ~7.5 MiB (`collect(upTo: 10_485_760)`,
the reported "8 MB cap"), Windows/Kestrel at ~21 MiB, Linux/`http.server`
unbounded.

## Decision

Switch `/upload` and `/download` to **raw binary streaming** over
`application/octet-stream`, with the destination/source path carried as a
percent-encoded `path` query parameter:

- `POST /upload?path=<percent-encoded>` — request body is the raw file bytes.
  The agent streams the body into a temp file **in the destination's own
  directory**, then atomically renames it into place on success; any error
  unlinks the temp file. The destination path is never left truncated. Success
  returns the existing `ActionResponse` JSON; failure returns the existing
  `ErrorResponse` JSON (`upload_failed`) with a 4xx/5xx status.
- `POST /download?path=<percent-encoded>` — success streams the file as an
  `application/octet-stream` response body; failure returns `ErrorResponse`
  JSON (`download_failed`) distinguished by HTTP status.

No base64, no whole-file buffering on either end — memory use is bounded by a
fixed streaming buffer regardless of file size, in a single request.

This is a **hard coordinated cutover**: all three agents (Hummingbird, Kestrel,
`http.server`), both CLI clients (Rust `cli-rs`, Swift `cli`), this protocol's
doc, and the macOS agent's vendored `TestAnywareAgentProtocol` copy change
together; golden images are rebuilt. There is no version negotiation and no
base64 fallback — a mismatched CLI/agent pair fails with a clear error.

## Considered alternatives

- **Raise the macOS cap only** / **unify a documented cap across agents** —
  rejected: both keep the base64 inflation and the whole-file memory ceiling,
  just relocating or documenting it.
- **Chunked application-level protocol** (begin/chunk/commit with offsets) —
  rejected: resumability and progress reporting are not requirements, so the
  extra endpoints and per-request reassembly state are unjustified complexity.
- **Path in a custom header** (`X-Upload-Path`) — rejected: HTTP header values
  are ASCII/Latin-1 by spec, forcing a bespoke encoding for non-ASCII guest
  paths; query-param percent-encoding is standard and unambiguous.
- **Capability-negotiated or versioned-endpoint fallback** — rejected: keeps
  the base64 path (and its ceiling) alive as dead code in a soon-to-be-retired
  Swift CLI, for a monorepo with no external protocol consumers.

## Consequences

- Breaking wire change: a new CLI against a stale baked-in agent (or vice
  versa) fails. Golden images must be rebuilt as part of the change.
- The JSON `UploadRequest`/`DownloadRequest` (and the base64 `content` field on
  download's response) are removed from both protocol type sets.
- `file-upload.json`'s `bytes` receipt is still produced by the CLI from the
  local file size; no schema change needed.
