# CLI Design Contract (Rust port)

**Status:** Authoritative for the Rust port of `testanyware`. The Swift CLI
(`cli/Sources/testanyware/`) is being retired and is **not** being
refactored to meet this contract; everything below describes the target
shape that all Rust command ports must satisfy before merge.

**Authority:** This document is referenced by name from the acceptance
criteria of every downstream Rust-port backlog task. Any deviation must
be raised as a contract amendment first.

**Source guidance:** Distilled from
`/Users/antony/Development/Ravel-Lite/defaults/fixed-memory/cli-tool-design.md`
("CLI Design Guidelines for LLM Agents") plus the existing TestAnyware
surface in `cli/Sources/testanyware/` and
`docs/reference/error-codes.md`.

---

## 1. Verb/noun convention

**Decision: noun-first, with a curated set of verb-first top-level
shortcuts.**

Most subcommands are noun-first (`<noun> <verb>`):

| Group | Subcommands |
|---|---|
| `vm` | `start`, `stop`, `list`, `delete` |
| `agent` | `health`, `snapshot`, `inspect`, `press`, `set-value`, `focus`, `show-menu`, `windows`, `window-focus`, `window-resize`, `window-move`, `window-close`, `window-minimize`, `wait` |
| `input` | `key`, `key-down`, `key-up`, `type`, `click`, `mouse-down`, `mouse-up`, `move`, `scroll`, `drag` |
| `screen` | `capture`, `record`, `size`, `find-text` |
| `file` | `upload`, `download` |

**Verb-first top-level shortcuts are kept** for muscle memory and
discoverability. They are aliases of the noun-first canonical form, not
independent commands:

| Verb-first alias | Canonical |
|---|---|
| `screenshot` | `screen capture` |
| `record` | `screen record` |
| `screen-size` | `screen size` |
| `find-text` | `screen find-text` |
| `upload` | `file upload` |
| `download` | `file download` |
| `exec` | `file exec` (lives under `file` because it is an in-VM action; alias retained because it is heavily scripted) |

**Aliases policy.** Both forms are tested. `--help` for the alias must
say "Alias of `<canonical>`". Help and schema are emitted from the
canonical command — aliases do not re-document the surface. Removing an
alias is a breaking change.

**Common synonym aliases** (cheap; required):

| Canonical | Required aliases |
|---|---|
| `list` | `ls` |
| `delete` | `rm`, `remove` |
| `inspect` | `show` |

**Internal commands** (not part of the public contract): names prefixed
with `_` (e.g. `_server`). They MUST set `hide = true` in clap so they
do not appear in `--help` output. They are not bound by any contract
section below.

---

## 2. Flag vocabulary

**One name per concept. No synonyms across commands.** The Rust port
uses these and only these names:

| Concept | Long flag | Short | Notes |
|---|---|---|---|
| Structured output | `--json` | — | Bool flag; equivalent to `--output json`. |
| Output format selector | `--output <fmt>` | `-o` | Values: `text` (default), `json`, `jsonl`. `-o` is reserved here; commands MUST NOT use `-o` for output-file paths (see below). |
| Output **file** path | `--out <path>` | — | When a command writes a file, use `--out`, not `--output`. Disambiguates from the format selector. |
| Suppress non-essential stderr | `--quiet` | `-q` | Stderr-only effect; never silences errors. |
| Diagnostic detail | `--verbose` | `-v` | Repeatable (`-vv`) for higher levels. Stderr only. |
| Preview without side effects | `--dry-run` | — | Required on every mutating command (see §9). |
| Non-interactive confirm | `--yes` | `-y` | Bypasses confirmation prompts. |
| Override safety check | `--force` | — | Distinct from `--yes`. Required when a check actively refuses. |
| Narrow list output | `--filter <expr>` | — | Field=value pairs, comma-separated. Per-command grammar documented in help. |
| Field-specific filter | `--<field> <value>` | — | E.g. `--platform macos` on `vm list`. |
| Page size | `--limit <n>` | — | Default 100 on list commands. |
| Unbounded results | `--all` | — | Mutually exclusive with `--limit`. |
| Custom format template | `--format <tmpl>` | — | Optional; only where templating is meaningful. Must NOT collide with `--output <fmt>`. |
| Color | `--color <when>` | — | Values: `auto` (default), `always`, `never`. |

**Reserved short flags** (must remain consistent across the whole
binary): `-q`, `-v`, `-y`, `-o` (output format only), `-h` (help), `-V`
(version).

**Flags that MUST NOT appear** (deprecated synonyms of the above):
`--confirm`, `--noprompt`, `--silent`, `--debug`, `--no-color`,
`--pretty`, `--raw`. New commands MUST reuse the table above instead of
inventing.

**Connection flags** (orthogonal to the vocabulary; see §6 for
identifiers). Inherited unchanged from the Swift CLI:

`--connect <path>` | `--vm <id>` | `--vnc <host:port>` |
`--agent <host:port>` | `--platform <macos|linux|windows>`

These remain because they identify the target rather than configure
behaviour. Their resolution chain (`--connect` → `--vm` →
`--vnc`/`--agent`/`--platform` → `TESTANYWARE_VM_ID` → other env vars)
is part of the contract; see `docs/reference/connection-spec.md`.

---

## 3. JSON schema policy

### 3.1 Coverage

Every **data-producing** command MUST support `--json` (equivalently
`--output json`). "Data-producing" means: returns structured information
the caller will parse, including confirmations of mutations.

The current Rust-port target list:

| Command | Mode | Schema id |
|---|---|---|
| `screen capture` | data (file path + bytes) | `screen-capture` |
| `screen record` | data (file path + frames + duration) | `screen-record` |
| `screen size` | data | `screen-size` |
| `screen find-text` | data (array of detections) | `screen-find-text` |
| `vm start` | data | `vm-start` |
| `vm stop` | data (mutation receipt) | `vm-stop` |
| `vm list` | data | `vm-list` |
| `vm delete` | data (mutation receipt) | `vm-delete` |
| `agent health` | data | `agent-health` |
| `agent snapshot` | data | `agent-snapshot` |
| `agent inspect` | data | `agent-inspect` |
| `agent windows` | data | `agent-windows` |
| `agent press` / `focus` / `set-value` / `show-menu` / `wait` | data (action receipt) | `agent-action` |
| `agent window-*` | data (action receipt) | `agent-window-action` |
| `input *` | data (action receipt) | `input-action` |
| `file exec` | data | `file-exec` |
| `file upload` | data (mutation receipt) | `file-upload` |
| `file download` | data (mutation receipt) | `file-download` |
| `doctor` | data | `doctor` |
| `capabilities` | data | `capabilities` |

### 3.2 Stability

Schemas are versioned by the binary's major version. **Adding** a field
or a non-required enum variant is permitted in any release. **Removing**
or **renaming** a field, or changing its type, requires a major version
bump and a documented migration path.

Every JSON object MUST include `"schema_version"` (string, semver of the
schema). Streaming output (`--output jsonl`) emits `schema_version` on
each line.

### 3.3 Schema directory

JSON schemas live at `docs/reference/cli-schemas/<schema-id>.json` in
[JSON Schema 2020-12](https://json-schema.org/draft/2020-12/release-notes)
format. The directory is canonical: a command's actual `--json` output
MUST validate against its declared schema in CI.

Stub schemas (objects with only `schema_version` and `$comment: "TODO"`)
are acceptable for commands that have not yet been ported, so the tree
of schema files stays parallel to the command tree from day one.

### 3.4 Errors in JSON mode

When `--json` (or `--output json|jsonl`) is set and a command fails, the
**only** thing on stdout is a single JSON error object. Stderr is empty
or carries diagnostic detail under `--verbose`, never user-facing prose.

```json
{
  "schema_version": "1.0",
  "ok": false,
  "code": "VM_NOT_FOUND",
  "message": "No spec found for VM id 'testanyware-deadbeef'",
  "remediation": "Start a VM with `testanyware vm start` or check the id with `testanyware vm list --ls --json`.",
  "details": { "vm_id": "testanyware-deadbeef", "spec_path": "/Users/.../vms/testanyware-deadbeef.json" }
}
```

`code` is one of the stable strings catalogued in §4. `details` is a
free-form object; its keys are documented per error code where useful.

### 3.5 Truncation

List commands MUST signal truncation in the JSON envelope:

```json
{
  "schema_version": "1.0",
  "ok": true,
  "items": [...],
  "returned": 100,
  "total": 1438,
  "truncated": true
}
```

Human output prints a final line: `Showing 100 of 1,438. Use --limit N
or --all to see more.`

### 3.6 Streaming

For commands that emit a long stream (currently `screen find-text
--watch`, future `agent snapshot --watch`), use `--output jsonl`. One
JSON object per line, each with `schema_version`. Do **not** use a
giant top-level array — agents and downstream tools should be able to
process incrementally.

---

## 4. Error codes

**Codes are stable strings.** Renaming or removing one is a major
version bump.

### 4.1 Authentication, connection, target resolution

| Code | When |
|---|---|
| `AUTH_REQUIRED` | The agent rejected the request because TCC / accessibility permission is not granted. |
| `CONNECTION_REFUSED` | TCP connect to VNC or agent failed; service likely not listening. |
| `CONNECTION_TIMEOUT` | TCP connect or RFB handshake exceeded the timeout. |
| `CONNECTION_DROPPED` | Peer dropped during a request. |
| `INVALID_ENDPOINT` | `--vnc`/`--agent` value parsed empty or out-of-range. |
| `NO_CONNECTION_SPECIFIED` | None of `--connect`, `--vm`, `--vnc`, `TESTANYWARE_VM_ID`, `TESTANYWARE_VNC` resolved. |
| `INVALID_PLATFORM` | `--platform` not in `{macos, linux, windows}`. |

### 4.2 VM lifecycle

| Code | When |
|---|---|
| `VM_NOT_FOUND` | `--vm <id>` resolved to a missing spec. |
| `VM_BOOT_TIMEOUT` | VNC didn't become reachable within the boot window. |
| `VM_STOP_FAILED` | `vm stop` couldn't terminate the VM cleanly. |
| `VM_BACKEND_UNSUPPORTED` | Neither tart nor QEMU can serve the requested platform on this host. |
| `GOLDEN_NOT_FOUND` | Requested golden image name doesn't exist. |
| `GOLDEN_IN_USE` | `vm delete` refused: running clones depend on the image. Use `--force`. |
| `TART_FAILED` | tart subprocess returned non-zero. `details.tart_error` carries its stderr. |
| `QEMU_FAILED` | QEMU subprocess failed (start, monitor, or QMP). |
| `KVM_PERMISSION_DENIED` | `/dev/kvm` is missing or not readable+writable (Linux host). `details.path` carries `/dev/kvm`. |
| `SWTPM_MISSING` | swtpm is not installed; required for Windows guests (TPM 2.0 socket). |
| `UEFI_NOT_FOUND` | UEFI firmware path missing for the requested QEMU configuration. |
| `SPAWN_FAILED` | `posix_spawn` (or platform equivalent) returned an error. |

> **Amendment 2026-05-22** (`port-qemu-runner-and-vm-lifecycle-to-rust`):
> `KVM_PERMISSION_DENIED` and `SWTPM_MISSING` added to support the QEMU
> runner's host preflight. `KVM_PERMISSION_DENIED` maps to exit code `4`
> (permission family, §5); `SWTPM_MISSING` maps to exit code `1` (generic
> — a missing optional dependency, recoverable by installing it).

### 4.3 VNC capture

| Code | When |
|---|---|
| `VNC_NOT_CONFIGURED` | Capture used before `connect()` succeeded. |
| `VNC_FRAMEBUFFER_NOT_READY` | Screenshot requested before a full framebuffer update arrived. |
| `VNC_CAPTURE_FAILED` | RFB-side capture failed (server-rejected encoding, etc.). |
| `VNC_ENCODING_FAILED` | Output encoding (PNG/H.264/HEVC) failed. |
| `VNC_PIXEL_MISMATCH` | Received byte count doesn't match advertised framebuffer size. |
| `VNC_DIMENSIONS_ZERO` | Server advertised a zero-dimension framebuffer. |

### 4.4 Recording

| Code | When |
|---|---|
| `RECORD_ALREADY_ACTIVE` | `record` invoked while a recording is already running on this VM. |
| `RECORD_NOT_ACTIVE` | Stop / append called outside a recording. |
| `RECORD_BUFFER_UNAVAILABLE` | AVAssetWriter pixel-buffer pool not ready. |
| `RECORD_BUFFER_CREATE_FAILED` | `CVPixelBufferCreate` (or libav equivalent) returned non-success. |

### 4.5 Agent

The three agent implementations historically use different error
strings (per `docs/reference/error-codes.md`). The contract closes this:
**agents must emit `error` strings drawn from the table below**, and
the host CLI maps them 1:1 to a `code`.

| Wire `error` (agent → host) | CLI `code` | Meaning |
|---|---|---|
| `not_found` | `ELEMENT_NOT_FOUND` | Element query matched zero elements. |
| `ambiguous` | `ELEMENT_AMBIGUOUS` | Multiple matches; supply `--index N` or narrow filter. |
| `window_not_found` | `WINDOW_NOT_FOUND` | No window matched the `--window` filter. |
| `action_unsupported` | `ACTION_UNSUPPORTED` | Element matched but does not support the requested action. |
| `accessibility_unavailable` | `AUTH_REQUIRED` | OS-level accessibility disabled or not yet granted. |
| `exec_failed` | `EXEC_FAILED` | Process failed to spawn (exit codes are returned in the receipt, not via this error). |
| `upload_failed` | `UPLOAD_FAILED` | File I/O error on the VM during upload. |
| `download_failed` | `DOWNLOAD_FAILED` | File I/O error on the VM during download. |
| `invalid_json` | `AGENT_INVALID_JSON` | Agent could not parse the request body as JSON. Indicates a CLI bug — agents do not generate this for valid client traffic. |

The Linux and Windows agent ports MUST be brought into line with the
table above as part of the per-platform agent backlog work; the host
CLI MUST NOT carry a translation map. Today's wrapper logic in
`AgentTCPClient` (which carries the wire `error` string verbatim into
the displayed message) is replaced by a strict mapping; unknown wire
strings surface as `code: AGENT_ERROR_UNKNOWN` with the wire string in
`details.wire_error`.

### 4.6 Generic / fall-through

| Code | When |
|---|---|
| `USAGE_ERROR` | clap-level: bad flag, missing arg, invalid combination. |
| `IO_ERROR` | Local filesystem error (permission denied, disk full, etc.). `details.path` carries the offending path. |
| `OCR_UNAVAILABLE` | EasyOCR daemon cannot be recovered. Override with `TESTANYWARE_OCR_FALLBACK=1`. |
| `OCR_CHILD_CRASHED` | EasyOCR daemon subprocess crashed. |
| `OCR_TIMEOUT` | EasyOCR daemon did not respond within the timeout. |
| `UNKNOWN_KEY` | Key name not in the supported set. |
| `UNKNOWN_BUTTON` | Mouse button name not in `{left, right, middle, center}`. |
| `INTERNAL` | The CLI itself failed in a way that should not happen. Should always be a bug; `details.trace_id` for log correlation. |

### 4.7 Discoverability errors

| Code | When |
|---|---|
| `SCHEMA_NOT_FOUND` | `testanyware schema <command>` invoked with an unknown command, or with a command that has no declared schema. Exit code `3` (per §5). |

### 4.8 Catalogue exposure

`testanyware capabilities --json` (see §8) MUST emit the full code
catalogue in `error_codes`. Adding a new code in a release means the
catalogue grows by one entry; agents can poll for additions.

---

## 5. Exit code scheme

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | Generic failure (no more specific bucket fits). |
| `2` | Usage error (`USAGE_ERROR` family — bad flags, missing args, invalid combinations). |
| `3` | Not found (`*_NOT_FOUND` family: `VM_NOT_FOUND`, `WINDOW_NOT_FOUND`, `ELEMENT_NOT_FOUND`, `GOLDEN_NOT_FOUND`, `UEFI_NOT_FOUND`, `SCHEMA_NOT_FOUND`). |
| `4` | Auth/permission required (`AUTH_REQUIRED`, `KVM_PERMISSION_DENIED`). |
| `5` | Conflict / precondition failed (`GOLDEN_IN_USE`, `RECORD_ALREADY_ACTIVE`, `ELEMENT_AMBIGUOUS`, `ACTION_UNSUPPORTED`). |
| `6` | Rate limited / try again later (reserved; not currently used). |
| `7` | Boot/startup timeout (`VM_BOOT_TIMEOUT`, `CONNECTION_TIMEOUT`, `OCR_TIMEOUT`, recording-related timeouts). |

**Sub-process exit codes.** `file exec` propagates the in-VM process's
exit code through `details.exit_code` in JSON mode and via the binary's
own exit code in text mode (today's behaviour, kept). The binary's exit
code is only `0` on a clean spawn AND a clean in-VM exit.

**Documented in help.** Every subcommand's `--help` lists the exit
codes it can produce (see §7).

---

## 6. Identifier conventions

| Identifier | Format | Round-trippable? |
|---|---|---|
| VM instance id | `testanyware-<8 hex>` | Yes — accepted by `--vm`, returned by `vm start` / `vm list --json`. |
| Golden image name | `testanyware-golden-<platform>-<release>` | Yes — accepted by `vm delete` / passed via `--base`. |
| AX element id | Agent-issued opaque string | Yes — accepted by `agent --id <id>`, returned by `agent inspect --json`. |
| Window id | Agent-issued opaque string | Yes — accepted by `agent --window <filter>` (currently substring-matched on title/app, see Gap §G.4). |

### 6.1 Round-trip rule

Whatever JSON output a command emits, every `id`, `vm_id`,
`golden_name`, etc. field MUST be accepted unchanged as input to a
related command. Tests in CI MUST exercise this round-trip for every
identifier-bearing command pair.

### 6.2 Stable across calls

VM ids and golden names persist across invocations. AX element ids and
window ids are session-scoped and MAY change between snapshots — agents
that need persistence across snapshots MUST resolve by `(role, label,
window)` rather than by id.

### 6.3 No bare integers

Every identifier in JSON output is a string with a typed prefix. Bare
integer ids are forbidden for new commands.

---

## 7. Help-text template

Every subcommand's `--help` MUST emit, in order:

1. **One-line summary** (≤ 80 chars). This is the `about`.
2. **Description** — one paragraph: when to use it, when not to, key
   caveats.
3. **Synopsis** — the canonical invocation.
4. **Arguments and flags** — grouped: required, common options,
   target-resolution (`--connect`/`--vm`/...), output options
   (`--json`/`--output`/`--out`), advanced.
5. **Output formats** — explicit list of stable outputs: e.g. "Stable:
   `--json`. Human output is not a parsing target."
6. **Exit codes** — list every code this subcommand can produce.
7. **Examples** — **at least 2 concrete invocations**, ideally 3,
   covering the most common real uses. Hard requirement; PR review
   blocks on this.
8. **See also** — sibling subcommands and related commands.

### 7.1 Example template skeleton

```
Capture a screenshot from the VNC server.

Connects to the VNC channel of the target VM, captures the current
framebuffer, and writes a PNG. Use --region for crop, --window to crop
to a named window's bounds, or neither for the full screen.

USAGE:
    testanyware screen capture [OPTIONS]

TARGET (one required):
    --connect <PATH>     Connection spec JSON file
    --vm <ID>            VM instance id (testanyware-<hex8>)
    --vnc <HOST:PORT>    VNC endpoint
    --agent <HOST:PORT>  Agent endpoint (host:port, default port 8648)

OPTIONS:
    --out <PATH>         Output PNG path [default: screenshot.png]
    --region <X,Y,W,H>   Crop region in pixels
    --window <NAME>      Crop to the bounds of the named window (resolved via agent)
    -o, --output <FMT>   Output format: text|json [default: text]
        --json           Shortcut for --output json
    -q, --quiet
    -v, --verbose

OUTPUT:
    Stable formats: --json (schema: screen-capture).
    Text output is not a parsing target.

EXIT CODES:
    0  success
    1  generic failure (e.g. VNC_CAPTURE_FAILED)
    2  usage error
    3  VM_NOT_FOUND, WINDOW_NOT_FOUND
    4  AUTH_REQUIRED (accessibility permission needed for --window)
    7  CONNECTION_TIMEOUT

EXAMPLES:
    # Capture full screen of a running VM
    testanyware screen capture --vm "$TESTANYWARE_VM_ID" --out screen.png

    # Capture only the Settings window
    testanyware screen capture --vm "$TESTANYWARE_VM_ID" --window Settings --out settings.png

    # JSON receipt (path + bytes) for scripting
    testanyware screen capture --vm "$TESTANYWARE_VM_ID" --out screen.png --json

SEE ALSO:
    testanyware screen size, testanyware screen record, testanyware screen find-text
```

### 7.2 Aliases

The alias's `--help` emits a single line `Alias of \`testanyware screen
capture\`. Run that for full help.` and clap's defaults for short help.
This avoids duplication.

---

## 8. Discoverability commands

### 8.1 `testanyware capabilities`

**Required.** Stable JSON describing the binary's surface. Agents poll
this to detect feature availability without parsing help.

```json
{
  "schema_version": "1.0",
  "version": "0.7.2",
  "git_revision": "9d8400d",
  "subcommands": ["vm", "agent", "input", "screen", "file", "doctor", "capabilities", "schema", "llm-instructions"],
  "aliases": { "screenshot": "screen capture", "record": "screen record", "find-text": "screen find-text", "screen-size": "screen size", "upload": "file upload", "download": "file download", "exec": "file exec" },
  "output_formats": ["text", "json", "jsonl"],
  "features": {
    "idempotency_keys": false,
    "streaming": true,
    "dry_run": true,
    "schema_command": true
  },
  "platforms": { "host": ["macos", "linux"], "guest": ["macos", "linux", "windows"] },
  "error_codes": ["AUTH_REQUIRED", "CONNECTION_REFUSED", "VM_NOT_FOUND", ...]
}
```

Default output is JSON (this is a machine-only command). `--output
text` may pretty-print a summary; humans typically use `--help`
instead.

### 8.2 `testanyware schema <command>`

**Required.** Emits the JSON Schema for `<command> --json` output.
Reads from `docs/reference/cli-schemas/<schema-id>.json` (embedded at
build time via `include_str!`).

```
testanyware schema vm list
testanyware schema agent snapshot
```

Exit `3` with `code: SCHEMA_NOT_FOUND` (catalogued in §4.7) when the
command is unknown or has no declared schema.

### 8.3 `testanyware llm-instructions`

**Required.** The full LLM usage guide for the binary, emitted as plain
text on stdout. This command is the single source of truth: it embeds
the repo-root `LLM_INSTRUCTIONS.md` at build time (`include_str!`), so an
LLM agent that has only the installed binary — e.g. a Homebrew install
with no source checkout — can read or prepend the complete reference.

It covers:

- One-paragraph "what this tool is and isn't."
- Mental model: noun-first command tree, verb-first aliases, the
  resolution chain (`--connect` → `--vm` → ...).
- Command reference for `vm`, `agent`, `input`, `screen`, `file`.
- End-to-end workflow recipes (discover-then-act, visual verification).
- Common mistakes:
  - "Don't parse text output; use `--json`."
  - "Don't click stale pixel coordinates; re-snapshot or filter by window."
  - "Don't poll `agent windows` in a tight loop; use `agent wait`."
- Authentication and state assumptions.
- JSON output, exit codes, and idempotency conventions.

Every instruction MUST be runnable with only the installed binary — no
source checkout, no repo-relative paths. Keep the guide lean enough to
prepend as LLM context (well under 3000 tokens); if it must grow
substantially, add `--topic <name>` / `--section <name>` flags rather
than introducing subcommands.

---

## 9. Behaviour invariants

These are enforced by the contract on top of the structural rules above.

### 9.1 TTY adaptation

Every command MUST detect whether stdout is a TTY and adapt:

- No pagers, no spinners, no carriage-return animations when piped or
  captured.
- No ANSI color unless `--color always` or stdout is a TTY and
  `--color` defaults to `auto`.
- No interactive prompts. Destructive operations must accept `--yes` /
  `--force` and MUST refuse to prompt when stdin is non-TTY (exit `2`
  with `USAGE_ERROR`, message naming the right flag).

### 9.2 Idempotency and retry safety

Every mutating command's `--help` documents:

- Whether running it twice equals running it once.
- Whether it is safe to retry on transient failure.
- Whether it has partial-failure semantics.

| Command | Idempotent? | Retry-safe? |
|---|---|---|
| `vm start` | Yes (returns existing id if `--id` matches a running VM; otherwise creates a new one). | Yes. |
| `vm stop` | Yes (no-op if already stopped). | Yes. |
| `vm delete` | Yes (no-op if absent). | Yes. |
| `screen capture` | Yes. | Yes. |
| `screen record` | **No** (creates a fresh recording each call). | **No** — concurrent recordings against the same VM produce `RECORD_ALREADY_ACTIVE`. |
| `agent press` / `set-value` / `focus` / `show-menu` | **No** (intent-level UI actions; double-press has app-defined effect). | **Application-defined.** Help must say "retry only if the previous attempt's outcome is unknown." |
| `agent window-*` | Yes (idempotent in target state). | Yes. |
| `input *` | **No.** Pressing a key twice is two presses. | **No.** |
| `file exec` | Application-defined. | Application-defined. |
| `file upload` / `download` | Yes (full overwrite). | Yes. |

Idempotency keys are reserved (`--idempotency-key <uuid>`) but not
implemented in v1.

### 9.3 `--dry-run` coverage

Required on every mutating command:

`vm start`, `vm stop`, `vm delete`, `screen record`, `agent press`,
`agent set-value`, `agent focus`, `agent show-menu`, `agent window-*`,
`input *`, `file exec`, `file upload`, `file download`.

Read-only commands (`vm list`, `screen capture`, `screen size`,
`screen find-text`, `agent health`, `agent snapshot`, `agent inspect`,
`agent windows`, `agent wait`, `doctor`, `capabilities`, `schema`,
`llm-instructions`) do not need `--dry-run` and MUST NOT advertise it.

Dry-run output validates inputs, resolves the connection, and emits the
intended action without performing it. Exit code `0` on a successful
plan; the same JSON envelope as the real run with `"dry_run": true` set.

### 9.4 Default list limits

`vm list`, `agent windows`, `agent snapshot` (when emitting flat
element lists), and `screen find-text` (when no query is supplied)
default to `--limit 100`. `--all` is required to opt into unbounded
output. Truncation is signalled per §3.5.

### 9.5 Hidden state

Any environment variable that influences behaviour MUST be listed in
`docs/reference/env-vars.md` AND surfaced in `capabilities --json`
under `env_vars`. Errors that result from a missing env var MUST name
that env var in `remediation`.

---

## 10. Gap report — Swift CLI vs. this contract

This is the audit deliverable. Each row lists what the Rust port must
do differently from the Swift original. The Swift CLI is being
retired, so these are not bugs to fix in `cli/` — they are
specifications for the Rust port.

### 10.1 Output

| Swift command | Gap | Rust target |
|---|---|---|
| `screenshot` | No `--json`. Confirmation prose only. | Add `--json` with `screen-capture` schema (path, bytes, region). Drop "Screenshot saved to X (N bytes)" prose; emit only on text mode. |
| `screen-size` | No `--json`. Emits `1920x1080` only. | Add `--json` (object with `width`, `height`). Keep `WxH` text shorthand for humans. |
| `record` | No `--json`. Emits status prose. | Add `--json` (path, fps, region, duration, frame count). |
| `find-text` | **Always emits JSON** even in text mode. No human alternative. Throws `ValidationError` when text not found rather than emitting an empty result. | Default to text mode (one detection per line); JSON is opt-in via `--json`. Empty result is exit `0` with empty array (text mode: silent), not an error — except when `--require-match` is set, which then exits `3` with `TEXT_NOT_FOUND`. |
| `exec` | Mixes stdout/stderr capture with prose; exit code via `ExitCode(result.exitCode)`. | Keep stdout/stderr passthrough in text mode. In `--json` mode emit a single object; do not interleave stdout. |
| `upload` / `download` | Confirmation prose only. | Add `--json` with `file-upload` / `file-download` schemas. |
| `vm list` | Hand-formatted via `VMListFormatter.render`. No `--json`. | Add `--json` (envelope per §3.5). Add `--filter platform=macos`, `--limit`, `--all`. |
| `agent windows` | Formatted text only. | Add `--json`. Truncation envelope per §3.5. |
| `agent press` / `set-value` / `focus` / `show-menu` / `wait` / `window-*` | Formatted text only. | Add `--json` with `agent-action` / `agent-window-action` schemas. |
| `agent health` | Prints `OK`/`UNHEALTHY`, throws `ExitCode.failure` on bad. | Keep text behaviour; add `--json` returning `{ok, agent_version, accessibility_status, uptime_s}`. |
| `doctor` | Decorated multi-line prose with ✓/✗/!. | Add `--json`. The five check groups become structured records. Keep the pretty text mode; have it render the same data. |

### 10.2 Help

| Swift command | Gap | Rust target |
|---|---|---|
| **All subcommands** | One-line `abstract` only; no examples, no exit codes section, no output-format declaration, no see-also. | Apply the §7 template uniformly. CI gate: every public subcommand's help text contains `EXAMPLES:` and at least 2 example invocations. |
| `vm delete` | No mention of `--force` semantics in help. | Help describes when `--force` is required (`GOLDEN_IN_USE`) and links to `vm list`. |
| `agent snapshot --open-menu` | Long inline help paragraph in `InputCommand.swift`. | Move the long-form explanation to `docs/reference/cli-commands.md`; help summarises and points there. |

### 10.3 Errors

| Swift command | Gap | Rust target |
|---|---|---|
| **Whole CLI** | No structured `code` field. Errors surface as `LocalizedError` text only. | Every error is a `(code, message, remediation, details)` tuple. JSON mode emits the structure; text mode renders it. |
| Connection resolution | Error mentions only `scripts/macos/vm-start.sh`. | `NO_CONNECTION_SPECIFIED` remediation lists every accepted form (flag, env var) and points to `docs/reference/connection-spec.md`. |
| `vm delete` (running clones) | `runningClonesPresent` doesn't expose the clone PIDs in a structured form. | `GOLDEN_IN_USE` JSON `details` includes `clone_ids` and `clone_pids`. |
| Agent errors | Three implementations emit different `error` strings (per `error-codes.md`). | Linux and Windows agents brought into line with §4.5 table; host CLI maps strictly. |

### 10.4 Conventions

| Swift | Gap | Rust target |
|---|---|---|
| Verb/noun mixed: `screenshot`, `record`, `find-text`, `screen-size`, `exec`, `upload`, `download` are top-level verbs; `vm`, `agent`, `input` are noun-first. | Inconsistent. | Canonical = noun-first per §1; verb-first names retained as aliases. |
| No aliases (`vm list` ≠ `vm ls`, `vm delete` ≠ `vm rm`). | Pure miss. | Add `ls`, `rm`/`remove`, `show` aliases per §1. |
| Flag inconsistency: `--detach`, `--viewer`, `--force`, ad-hoc per command. | No consolidation. | Reduce to the §2 table. `--detach` on `exec` stays (concept-specific); `--viewer` on `vm start` stays (concept-specific). New flags must reuse §2. |
| VM ids are already `testanyware-<hex>` prefix-typed. | Already compliant. | Keep. Document round-trip explicitly. |
| Window matching is substring case-insensitive across title and app — easy to over-match. | Loose. | Document explicitly in `--window` help; add `--window-regex` for exact-match scripts. (Optional v1; track in backlog.) |

### 10.5 Behaviour

| Swift | Gap | Rust target |
|---|---|---|
| No TTY detection. `print()` always. | Pure miss. | TTY-aware output, color, and progress per §9.1. |
| `--dry-run` not present anywhere. | Pure miss. | Add to all mutating commands per §9.3. |
| `--yes` not present (only `--force` on `vm delete`). | Partial. | `--yes` separate from `--force` per §2; required where prompts would otherwise appear. |
| `vm list` has no limit. | Unbounded. | `--limit 100` default per §9.4. |
| `record --duration 0` quietly maps to 300s. | Surprising. | Emit a `--verbose` notice; document explicitly; consider `--duration max` instead. |

### 10.6 Discoverability

| Swift | Gap | Rust target |
|---|---|---|
| No `capabilities` command. | Pure miss. | Add per §8.1. |
| No `schema <cmd>`. | Pure miss. | Add per §8.2. |
| No `llm-instructions`. | Pure miss. | Add per §8.3. |
| `--version` exists (via clap/ArgumentParser). | Compliant. | Keep; also expose under `capabilities --json`. |

---

## 11. Acceptance for downstream port tasks

Each Rust-port backlog task that adds or moves a command MUST verify
in its acceptance criteria:

1. The command's `--help` follows §7 (one-line, description, synopsis,
   flags, output formats, exit codes, ≥ 2 examples, see-also).
2. The command supports `--json` if it produces data, and the JSON
   output validates against `docs/reference/cli-schemas/<schema-id>.json`.
3. Errors surface a `code` from §4 and an exit code from §5.
4. Mutating commands have `--dry-run` (§9.3) and document idempotency
   (§9.2).
5. Identifiers in JSON output round-trip back as input to a sibling
   command (§6.1).
6. The command appears in `capabilities --json` (§8.1) and has a schema
   file at `docs/reference/cli-schemas/<schema-id>.json` even if
   stubbed.

The port-time CI gate runs `tests/cli-contract.rs` (to be added by the
first port task) which walks every public subcommand and asserts each
of these.

---

## 12. Out of scope

Listed here so they do not get added later by accident:

- Internationalisation. Help text is English-only.
- Localisation of error messages. Error `code` is the stable surface;
  `message` is a developer-language string.
- Hand-written shell completions. clap generates them from the same
  definitions.
- A REPL or interactive shell mode. The `llm-instructions` command
  covers the workflow guidance need.
- Plugin architecture. The contract assumes a single static binary.
