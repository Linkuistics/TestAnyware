# 040-windows-agent-streaming

**Kind:** work

## Goal

Convert the Windows agent's `/upload` and `/download` to raw streaming per
ADR-0001. Windows caps at the Kestrel default (~28.6 MiB body → ~21 MiB file);
move it to the octet-stream contract with no whole-file buffering.

## Context

- `agents/windows/SystemEndpoints.cs` — `app.MapPost("/upload", (UploadRequest
  req) => ...)` at ~79 (model-binds the JSON body, base64-decodes), and the
  `/download` handler. Built on **ASP.NET minimal API / Kestrel**.
- `agents/windows/Models/Requests.cs` — `UploadRequest` (~65); remove/repurpose.
- `agents/windows/Program.cs` — Kestrel host setup (no limits set today). If a
  body-size limit is needed at all it is now effectively removed by streaming;
  do not re-add a small `MaxRequestBodySize`.
- ADR-0001 + rewritten agent-protocol.md (leaf 010).

## Done when

- `/upload` takes `path` from the query string and streams `Request.Body` to a
  sibling temp file (`Stream.CopyToAsync`), then atomically moves into place
  (`File.Move(temp, dest, overwrite: true)`); errors delete the temp and return
  `ErrorResponse` (`upload_failed`).
- `/download` returns `Results.Stream`/`Results.File` with
  `application/octet-stream` streamed from disk; `ErrorResponse`
  (`download_failed`) on failure.
- `UploadRequest` model binding removed from the route signature.
- Windows agent builds (`dotnet build`); no small request-body cap reintroduced.

## Notes

Minimal-API handlers can take `HttpRequest`/`HttpContext` directly to access
`Request.Query["path"]` and `Request.Body`. Confirm `File.Move(..., overwrite)`
is atomic-enough on NTFS within one volume; the temp file must be on the same
volume as the destination.
