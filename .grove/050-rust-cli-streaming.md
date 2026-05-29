# 050-rust-cli-streaming

**Kind:** work

## Goal

Make the Rust CLI (`cli-rs`) upload/download stream from/to disk per ADR-0001,
removing the `std::fs::read` whole-file buffer and the base64 JSON body.

## Context

- `cli-rs/crates/testanyware-cli/src/commands/file.rs` — `run_upload` (~86)
  does `std::fs::read(local_path)` (~105) then `client.upload(&remote, &bytes)`
  (~119); `run_download` (~150) writes bytes with `std::fs::write`.
- `cli-rs/crates/testanyware-agent-client/` (the `AgentClient`) — the `upload`
  method that posts JSON. Switch to a streaming body (`reqwest::Body` from a
  file / `tokio::fs::File`, or a framed stream) with `?path=` query +
  `application/octet-stream`; `download` consumes the response body as a stream
  to the local file.
- `cli-rs/crates/testanyware-protocol/src/agent_requests.rs` — `UploadRequest`
  (~88) and `DownloadRequest`; remove/repurpose. Update `lib.rs` re-exports
  (~32) and the round-trip tests (~183).
- The `file-upload.json` receipt still reports `bytes` — derive from local file
  metadata (`std::fs::metadata().len()`), not from the buffer.
- ADR-0001 + rewritten agent-protocol.md (leaf 010).

## Done when

- Upload streams the local file as the request body (bounded memory) with the
  remote path percent-encoded into the `?path=` query; success/error mapped to
  the existing exit codes (`UPLOAD_FAILED`).
- Download streams the response body to the local path (write to temp +
  rename locally too, for symmetry/safety); `DOWNLOAD_FAILED` on error.
- `UploadRequest`/`DownloadRequest` JSON types removed; crate builds; tests
  updated; `--json` receipt still emits `bytes` + `ok`.
- `--dry-run` path unchanged in behavior.

## Notes

Check whether `reqwest` (or whatever HTTP client the agent-client crate uses) is
already a dependency and supports streaming bodies; percent-encode `path` with
the same scheme the agents decode with (leaf 010 pins it). Keep parity with the
Swift client (leaf 060) — both must speak the identical wire form.
