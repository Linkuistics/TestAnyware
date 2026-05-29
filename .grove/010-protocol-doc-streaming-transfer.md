# 010-protocol-doc-streaming-transfer

**Kind:** work

## Goal

Rewrite the `/upload` and `/download` sections of
`docs/architecture/agent-protocol.md` to specify the streaming contract from
ADR-0001, so the implementation leaves (020–060) build against one reference.
This is the contract; do it first.

## Context

- ADR-0001 (`docs/adr/0001-streaming-file-transfer.md`) — the normative source.
- `docs/architecture/agent-protocol.md` — the doc to edit. Today: `/upload`
  takes JSON `{path, content:<base64>}`; `/download` returns JSON
  `{content:<base64>}`. The endpoint table, "Request shapes" (`UploadRequest`,
  `DownloadRequest`), and "Response shapes" all mention the JSON forms.
- `docs/reference/error-codes.md` — confirm `upload_failed` / `download_failed`
  keys are documented; align if needed.

## Done when

- The endpoint table and request/response sections describe:
  `POST /upload?path=<percent-encoded>` with `application/octet-stream` raw body
  → `ActionResponse` on success, `ErrorResponse` (`upload_failed`) on failure;
  `POST /download?path=<percent-encoded>` → `application/octet-stream` body on
  success, `ErrorResponse` (`download_failed`) on failure.
- `UploadRequest` / `DownloadRequest` JSON request shapes and the base64
  `content` fields are removed (or marked removed) from the doc.
- Temp-file + atomic-rename write semantics and percent-encoding of `path` are
  stated.
- No stale "base64-encoded" language remains in the file-transfer sections.

## Notes

Doc-only leaf. The matching protocol *type* changes (removing
`UploadRequest`/`DownloadRequest` structs) happen in the per-client leaves
(050 Rust, 060 Swift) and the macOS vendored copy (020), not here.
