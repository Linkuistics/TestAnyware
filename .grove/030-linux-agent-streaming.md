# 030-linux-agent-streaming

**Kind:** work

## Goal

Convert the Linux agent's `/upload` and `/download` to raw streaming per
ADR-0001. Linux has no body cap today (stdlib `http.server` reads full
`Content-Length`), but it still buffers the whole base64 payload in memory and
must move to the new octet-stream contract for protocol consistency.

## Context

- `agents/linux/testanyware_agent/server.py` — `AgentRequestHandler`. Today
  `do_POST` calls `_read_body()` which reads the entire body and `json.loads`es
  it (line ~62), then dispatches `/upload` → `system_endpoints.handle_upload`.
  The route table is at ~30. Built on **`http.server.BaseHTTPRequestHandler`**
  (zero pip deps — keep it that way).
- `agents/linux/testanyware_agent/system_endpoints.py` — `handle_upload`
  (~53, base64-decodes `content`) and `handle_download` (~75, base64-encodes).
- ADR-0001 + rewritten agent-protocol.md (leaf 010).

## Done when

- `/upload` parses the `path` query param from `self.path`
  (`urllib.parse`), streams `self.rfile` to a sibling temp file in fixed-size
  chunks (read up to `Content-Length`), then atomically renames
  (`os.replace`); errors unlink the temp and return a JSON `upload_failed`
  error.
- `/download` streams the file to `self.wfile` in chunks with
  `Content-Type: application/octet-stream` and a `Content-Length`; JSON
  `download_failed` error on failure.
- The JSON-body read path is bypassed for these two endpoints (they are no
  longer JSON requests); other endpoints keep using `_read_body()`.
- No pip dependencies added; `agents/linux/tests/` pass / updated.

## Notes

`do_POST` currently assumes every route has a JSON body. The streaming
endpoints need to branch *before* `_read_body()` consumes the stream — route on
`self.path`'s path component first.
