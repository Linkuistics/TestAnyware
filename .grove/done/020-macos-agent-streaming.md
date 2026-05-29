# 020-macos-agent-streaming

**Kind:** work

## Goal

Convert the macOS agent's `/upload` and `/download` from base64-in-JSON to raw
streaming per ADR-0001, removing the `collect(upTo: 10_485_760)` cap that is the
reported "8 MB" limit.

## Context

- `agents/macos/Sources/testanyware-agent/AgentServer.swift` — `handleUpload`
  (line ~464), `handleDownload` (~480), the `UploadRequest` struct (~65), the
  route registrations (~95), and the offending `request.body.collect(upTo:
  10_485_760)` (~106). Built on **Hummingbird**.
- `agents/macos/Sources/TestAnywareAgentProtocol/` — the vendored protocol copy.
  Remove/adjust `UploadRequest`/`DownloadRequest` JSON types here to match the
  host side (kept as a separate copy; see agent-protocol.md "Why this contract
  exists as code on both sides").
- ADR-0001 + the rewritten agent-protocol.md (leaf 010).

## Done when

- `/upload` reads the `path` query param (percent-decoded) and streams the
  request body to a sibling temp file, then atomically renames into place;
  errors unlink the temp and return `ErrorResponse` (`upload_failed`).
- `/download` streams the file as `application/octet-stream`
  (Hummingbird `ResponseBody`/`AsyncSequence`), `ErrorResponse`
  (`download_failed`) on failure.
- No `collect(upTo:)` whole-body buffering on the upload path.
- Vendored protocol copy updated; macOS agent builds; existing tests pass / are
  updated.

## Notes

Investigate Hummingbird's streaming request-body API (`request.body` is an
async sequence of `ByteBuffer`) and streaming response bodies — avoid
re-introducing a full-buffer collect. Check the protocol-drift test referenced
in agent-protocol.md (`cli/Tests/TestAnywareAgentProtocolTests/`).
