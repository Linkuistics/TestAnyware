# Agent Protocol

The wire contract between the host CLI and the in-VM accessibility
agents. All three agents (macOS, Linux, Windows) implement the same
surface — the host driver is written once and targets whichever VM is
running.

This document is the **real contract**. Both sides must agree on
endpoint paths, request shapes, and response shapes, or the driver
breaks.

Authoritative sources (the JSON keys and optionality rules must match):

- Host-side Rust types: `cli-rs/crates/testanyware-protocol/`
  (`agent_requests.rs`, `element_info.rs`, `window_info.rs`,
  `unified_role.rs`).
- macOS agent's vendored copy of the same module:
  `agents/macos/Sources/TestAnywareAgentProtocol/`.
- Linux agent models: `agents/linux/testanyware_agent/models.py`.
- Windows agent models: `agents/windows/Models/*.cs`.

## Transport

- HTTP/1.1 with JSON request and response bodies (Content-Type
  `application/json`) for every endpoint **except file transfer**:
  `/upload` and `/download` stream raw bytes as
  `application/octet-stream` (see those endpoints below).
- Default bind: `0.0.0.0:8648` on the VM. The host connects via the
  VM's LAN IP (tart: `192.168.64.<n>`; QEMU: reachable over virtio-net).
- Every endpoint is **POST** except `GET /health`.
- Success is HTTP 2xx with an endpoint-specific response body.
- Failure is HTTP 4xx/5xx with an `ErrorResponse` body (see below).

## Endpoints

### System

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness + "accessibility is ready" check. Returns 200 with no required body once agent + AX are ready. |
| POST | `/exec` | Execute a command. Always 200; exit code is in the response body. |
| POST | `/upload?path=<percent-encoded>` | Stream a file to the VM filesystem (raw `application/octet-stream` body). |
| POST | `/download?path=<percent-encoded>` | Stream a file from the VM filesystem (raw `application/octet-stream` response). |
| POST | `/shutdown` | Ask the agent to terminate (used by test harnesses). |
| POST | `/debug/ax` | macOS only. Dump internal AX state for debugging. |

### Accessibility

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/windows` | List windows visible to the AX layer. |
| POST | `/snapshot` | Element tree snapshot, optionally filtered. |
| POST | `/inspect` | One-element detail (font, color, bounds). |
| POST | `/press` | Activate an element (semantic default action). |
| POST | `/set-value` | Set an element's value. |
| POST | `/focus` | Focus an element. |
| POST | `/show-menu` | Show an element's context menu. |
| POST | `/wait` | Poll until AX is ready (first boot). |
| POST | `/window-focus` | Raise/focus a window. |
| POST | `/window-resize` | Resize a window. |
| POST | `/window-move` | Move a window. |
| POST | `/window-close` | Close a window. |
| POST | `/window-minimize` | Minimize a window. |

## Request shapes

### `ElementQuery` — shared by `/inspect`, `/press`, `/set-value`, `/focus`, `/show-menu`

```json
{
  "role":   "button",       // optional; one of UnifiedRole values
  "label":  "Save",         // optional
  "window": "Settings",     // optional (title or app name)
  "id":     "button-42",    // optional platform-native identifier
  "index":  0               // optional 0-based disambiguator
}
```

### `SnapshotRequest` — `/snapshot`

```json
{
  "mode":   "full",       // or "interactive"
  "window": "Finder",     // optional
  "role":   "button",     // optional
  "label":  "Save",       // optional
  "depth":  3             // optional max tree depth
}
```

### `SetValueRequest` — `/set-value`

Extends `ElementQuery` with a required `value`:

```json
{
  "role":   "textfield",
  "window": "Login",
  "value":  "alice"
}
```

### `WindowTarget` — `/window-focus`, `/window-close`, `/window-minimize`

```json
{ "window": "Document.txt" }
```

### `WindowResizeRequest`

```json
{ "window": "Document.txt", "width": 1200, "height": 800 }
```

### `WindowMoveRequest`

```json
{ "window": "Document.txt", "x": 100, "y": 100 }
```

### `WaitRequest` — `/wait`

```json
{ "window": "Finder", "timeout": 30 }
```

### `ExecRequest` — `/exec`

```json
{ "command": "uname -a", "timeout": 60, "detach": false }
```

### File transfer — `/upload`, `/download`

`/upload` and `/download` do **not** take a JSON body. The
destination/source path travels as a single percent-encoded `path`
query parameter, and the file bytes stream raw over
`application/octet-stream`. There is no `UploadRequest` /
`DownloadRequest` type and no base64 — the payload is the file itself.

**`POST /upload?path=<percent-encoded>`** — the request body is the raw
file bytes (`Content-Type: application/octet-stream`). The agent streams
the body into a temp file **in the destination's own directory**, then
atomically renames it into place once the transfer completes; any error
unlinks the temp file, so the destination path is never left holding a
truncated file. Success returns `ActionResponse` (see below); failure
returns `ErrorResponse` (`upload_failed`) with a 4xx/5xx status.

**`POST /download?path=<percent-encoded>`** — on success the response
body is the raw file bytes (`Content-Type: application/octet-stream`);
on failure the response is an `ErrorResponse` (`download_failed`) JSON
body, distinguished from a successful transfer by its non-2xx HTTP
status.

`path` is percent-encoded per the standard URI query-component rules so
that Unicode and special characters in guest paths survive uniformly
across all three agent stacks. Neither end buffers the whole file:
memory use is bounded by a fixed streaming buffer regardless of file
size, in a single request.

## Response shapes

### `SnapshotResponse` — `/snapshot`, `/windows`

```json
{
  "windows": [ <WindowInfo>, ... ]
}
```

### `InspectResponse` — `/inspect`

Encodes `CGRect` as flat keys (`boundsX`, `boundsY`, `boundsWidth`,
`boundsHeight`) — all four present or all four absent.

```json
{
  "element":      <ElementInfo>,
  "fontFamily":   "SF Pro",
  "fontSize":     13,
  "fontWeight":   "regular",
  "textColor":    "#000000",
  "boundsX":      10,
  "boundsY":      20,
  "boundsWidth":  100,
  "boundsHeight": 24
}
```

### `ActionResponse` — `/press`, `/set-value`, `/focus`, `/show-menu`, `/window-*`, `/wait`, `/shutdown`, `/upload`

```json
{ "success": true, "message": "optional detail" }
```

### Exec response — `/exec`

Exec returns 2xx even on a non-zero exit. Callers must check `exitCode`.

```json
{
  "success":  true,
  "message":  null,
  "stdout":   "...",
  "stderr":   "...",
  "exitCode": 0
}
```

### `ErrorResponse` — any non-2xx

```json
{ "error": "<short key>", "details": "<optional human message>" }
```

Common `error` keys: `not_found`, `element_not_found`, `ambiguous`,
`multiple_matches`, `window_not_found`, `action_unsupported`,
`accessibility_unavailable`, `exec_failed`, `upload_failed`,
`download_failed`. Clients must tolerate unknown strings; see
`docs/reference/error-codes.md`.

## Nested types

### `WindowInfo`

```json
{
  "title":       "Document.txt",
  "windowType":  "regular",
  "sizeWidth":   1200,
  "sizeHeight":  800,
  "positionX":   100,
  "positionY":   100,
  "appName":     "TextEdit",
  "focused":     true,
  "elements":    [ <ElementInfo>, ... ]   // present only on /snapshot
}
```

`elements` is absent on `/windows` (headers-only listing) and present on
`/snapshot`. CGPoint / CGSize are split into flat keys.

### `ElementInfo`

```json
{
  "role":         "button",
  "label":        "Save",
  "value":        null,
  "description":  null,
  "id":           "btn-save",
  "enabled":      true,
  "focused":      false,
  "showing":      true,                   // optional; some agents omit
  "positionX":    0,
  "positionY":    0,
  "sizeWidth":    80,
  "sizeHeight":   24,
  "childCount":   0,
  "actions":      ["press", "focus"],
  "platformRole": "AXButton",
  "children":     [ <ElementInfo>, ... ]  // present only on /snapshot full-tree
}
```

### `UnifiedRole`

Cross-platform role vocabulary: agents map their native roles
(AXButton, UIA Button, ATK push button) onto this enum so the host can
write one set of selectors. Full list in
`cli-rs/crates/testanyware-protocol/src/unified_role.rs` — it covers
interactive widgets (`button`, `checkbox`, `textfield`, `slider`, ...),
menus (`menu`, `menu-item`, ...), containers (`window`, `dialog`,
`toolbar`, ...), content (`heading`, `text`, ...), transient surfaces
(`popover`, `toast`), and `unknown` as a catch-all.

## Why this contract exists as code on both sides

The macOS agent vendors a copy of `TestAnywareAgentProtocol` (the host
CLI has the same sources). We keep them as separate copies so the
agent builds standalone; the tradeoff is that any protocol change must
be applied to both copies. A test (`cli/Tests/TestAnywareAgentProtocolTests/`)
exists to catch drift at the encoding level.

Linux and Windows agents implement the same JSON shape in their
respective languages; they are not Swift — the "contract" is the JSON,
not the Swift types.
