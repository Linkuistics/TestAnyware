# Agent Protocol

The wire contract between the host CLI and the in-VM accessibility
agents. All three agents (macOS, Linux, Windows) implement the same
surface. One host driver targets whichever VM runs.

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
  VM's LAN IP: `192.168.64.<n>` under tart, or the virtio-net address under QEMU.
- Every endpoint is **POST** except `GET /health`.
- Success is HTTP 2xx with an endpoint-specific response body.
- Failure is HTTP 4xx/5xx with an `ErrorResponse` body (see below).

## Endpoints

### System

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness + "accessibility is ready" check. Returns 200 with no required body once agent + AX are ready. |
| POST | `/exec` | Execute a command. A valid request returns 200. The response body holds the exit code. |
| POST | `/upload?path=<percent-encoded>` | Stream a file to the VM filesystem (raw `application/octet-stream` body). |
| POST | `/download?path=<percent-encoded>` | Stream a file from the VM filesystem (raw `application/octet-stream` response). |
| POST | `/shutdown` | Ask the agent to terminate (used by test harnesses). |
| POST | `/debug/ax` | macOS only. Dump internal AX state for debugging. |

### Accessibility

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/windows` | List windows visible to the AX layer. |
| POST | `/snapshot` | Element tree snapshot, optionally filtered. |
| POST | `/inspect` | One-element detail (font, colour, bounds). |
| POST | `/press` | Activate an element (semantic default action). |
| POST | `/set-value` | Set an element's value. |
| POST | `/focus` | Focus an element. |
| POST | `/show-menu` | Show an element's context menu. |
| POST | `/wait` | Poll until AX is ready (first boot). |
| POST | `/window-focus` | Raise/focus a window. |
| POST | `/window-resize` | Resize a window. |
| POST | `/window-move` | Move a window. |
| POST | `/window-close` | Close a window. |
| POST | `/window-minimize` | Minimise a window. |

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
{ "command": "uname -a", "timeout": 60, "detach": false, "id": "run-7.step-2" }
```

The Linux agent reads the optional `id`. It has 1 to 64 characters from
`A-Z`, `a-z`, `0-9`, `.`, `_` and `-`. Without an `id`, the agent makes
one. An attached request returns 400 with `exec_failed` in three cases.
The command is not a string, the timeout is not a number, or the `id`
is not valid.

The Host CLI sends a random UUID as the `id` of each attached request. An
agent without the typed record ignores it. A detached request carries no
`id`.

### File transfer — `/upload`, `/download`

`/upload` and `/download` do **not** take a JSON body. The
destination/source path travels as a single percent-encoded `path`
query parameter, and the file bytes stream raw over
`application/octet-stream`. There is no `UploadRequest` /
`DownloadRequest` type and no base64 — the payload is the file itself.

**`POST /upload?path=<percent-encoded>`** — the request body is the raw
file bytes (`Content-Type: application/octet-stream`). The agent streams
the body into a temp file **in the destination's own directory**. When
the transfer completes, the agent renames the temp file into place
atomically. On any error, the agent unlinks the temp file. So the
destination path never holds a truncated file.

Success returns
`ActionResponse` (see below). Failure returns `ErrorResponse`
(`upload_failed`) with a 4xx/5xx status.

**`POST /download?path=<percent-encoded>`** — on success the response
body is the raw file bytes (`Content-Type: application/octet-stream`).
On failure the response is an `ErrorResponse` (`download_failed`) JSON
body. Its non-2xx HTTP status tells it apart from a successful transfer.

Each end percent-encodes `path` by the standard URI query-component
rules. So Unicode and special characters in guest paths survive in the
same way on all three agent stacks. Neither end buffers the whole file.
A fixed streaming buffer limits memory use in a single request,
whatever the file size.

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
Every agent sends the legacy fields:

```json
{ "exitCode": 0, "stdout": "...", "stderr": "..." }
```

The Linux agent also sends the typed exec record beside them. The record
holds the command `id`, the `outcome`, the exact `evidence` of each
stream, the `lifecycle` times, `remainingGroup`, `termination` and
`legacyProjection`. A key that does not apply holds `null`.

The legacy fields are a projection of the record. They keep the values of the old
Linux agent, so an old Host CLI reads the same reply. The record is
defined in `docs/specs/guest-command-lifecycle.md`, section "Exec
record". The Linux model is `ExecRecord` in
`agents/linux/testanyware_agent/models.py`.

Detached commands return only the legacy fields.

The Host CLI reads a reply without `outcome` as a legacy reply. It checks
every validity rule of the spec on a reply with `outcome`. A record that
breaks a rule gives the error `EXEC_RECORD_INVALID`, and the Host CLI never
reports it as a success. The Rust reader is
`cli-rs/crates/testanyware-protocol/src/exec_record.rs`.

### `ErrorResponse` — any non-2xx

```json
{ "error": "<short key>", "details": "<optional human message>" }
```

Common `error` keys: `not_found`, `element_not_found`, `ambiguous`,
`multiple_matches`, `window_not_found`, `action_unsupported`,
`accessibility_unavailable`, `exec_failed`, `upload_failed`,
`download_failed`. Clients must tolerate unknown strings. See
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

Cross-platform role vocabulary. Agents map their native roles
(AXButton, UIA Button, ATK push button) onto this enum. So the host can
write one set of selectors. Full list in
`cli-rs/crates/testanyware-protocol/src/unified_role.rs` — it covers
interactive widgets (`button`, `checkbox`, `textfield`, `slider`, ...),
menus (`menu`, `menu-item`, ...), containers (`window`, `dialog`,
`toolbar`, ...), content (`heading`, `text`, ...), transient surfaces
(`popover`, `toast`), and `unknown` as a catch-all.

## Why this contract exists as code on both sides

The macOS agent vendors a copy of `TestAnywareAgentProtocol` (the host
CLI has the same sources). We keep them as separate copies so the
agent builds standalone. The trade-off: each protocol change must go
into both copies. A test (`cli/Tests/TestAnywareAgentProtocolTests/`)
exists to catch drift at the encoding level.

Linux and Windows agents implement the same JSON shape in their
own languages. They are not Swift. The "contract" is the JSON,
not the Swift types.
