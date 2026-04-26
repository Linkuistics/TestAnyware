---
title: CLI Commands
---

# CLI Commands Reference

Exhaustive per-subcommand reference for `testanyware`. Generated from
`swift run testanyware <cmd> --help`. Every command accepts these common
connection flags (resolved in order: `--connect` → `--vm` → explicit
`--vnc`/`--agent`/`--platform` → `TESTANYWARE_VM_ID` → `TESTANYWARE_VNC`
→ error):

- `--connect <path>` — Path to connection spec JSON file.
- `--vm <id>` — VM instance id; resolves to
  `$XDG_STATE_HOME/testanyware/vms/<id>.json`.
- `--vnc <host:port>` — VNC endpoint (default port `5900`).
- `--agent <host:port>` — Agent HTTP endpoint (default port `8648`).
- `--platform <macos|windows|linux>` — Target platform.
- `--version` / `-h, --help` — Standard flags on every subcommand.

The tables below omit those five flags on each row; assume they are
always available.

---

## Top-level subcommands

| Command | One-liner |
|---------|-----------|
| `screenshot` | Capture a screenshot from the VNC server |
| `screen-size` | Query the VNC display dimensions |
| `input` | Send keyboard and mouse input (sub-sub-commands) |
| `exec` | Execute a command on the VM via agent |
| `upload` | Upload a file to the VM via agent |
| `download` | Download a file from the VM via agent |
| `record` | Record VNC screen to a video file |
| `find-text` | Find text on screen using OCR (Vision on macOS, EasyOCR daemon on Linux/Windows) |
| `agent` | Interact with the in-VM accessibility agent (sub-sub-commands) |
| `vm` | VM lifecycle: start, stop, list, delete (sub-sub-commands) |

---

## `testanyware screenshot`

**Synopsis:** Capture a screenshot from the VNC server.

```
testanyware screenshot [--output <output>] [--region <region>] [--window <window>]
```

- `-o, --output <output>` — Output file path. Default: `screenshot.png`.
- `--region <region>` — Crop region as `x,y,width,height`.
- `--window <window>` — Window name for relative coordinates (crops to window
  bounds when no `--region` specified).

**Example:**
```bash
testanyware screenshot --vm "$TESTANYWARE_VM_ID" -o desktop.png
testanyware screenshot --vm "$TESTANYWARE_VM_ID" --region 0,0,800,600 -o top-left.png
```

---

## `testanyware screen-size`

**Synopsis:** Query the VNC display dimensions. Prints `WxH` to stdout.

```
testanyware screen-size
```

**Example:**
```bash
testanyware screen-size --vm "$TESTANYWARE_VM_ID"   # → "1920x1080"
```

---

## `testanyware input`

Container for keyboard and mouse primitives. Sub-sub-commands below.

### `testanyware input key`

**Synopsis:** Press a key (down + up).

```
testanyware input key <key> [--modifiers <modifiers>]
```

- `<key>` — Key name (see [key-names.md](key-names.md)).
- `-m, --modifiers <modifiers>` — Comma-separated: `cmd,shift,alt,ctrl`.

**Example:**
```bash
testanyware input key return
testanyware input key a --modifiers cmd         # Cmd-A (select all on macOS)
testanyware input key z --modifiers cmd,shift   # Cmd-Shift-Z (redo)
```

### `testanyware input key-down`

**Synopsis:** Send key-down without releasing. Pair with `input key-up`.

```
testanyware input key-down <key>
```

### `testanyware input key-up`

**Synopsis:** Send key-up (release).

```
testanyware input key-up <key>
```

### `testanyware input type`

**Synopsis:** Type text literally (handles case and shifted symbols).

```
testanyware input type <text>
```

**Example:**
```bash
testanyware input type "Hello, World!"
```

### `testanyware input click`

**Synopsis:** Click at coordinates.

```
testanyware input click <x> <y> [--button <button>] [--count <count>] [--window <window>]
```

- `<x> <y>` — Screen-absolute coordinates (or window-relative with `--window`).
- `-b, --button <button>` — `left`, `right`, `middle`. Default: `left`.
- `-c, --count <count>` — Click count (1 = single, 2 = double). Default: `1`.
- `--window <window>` — Window name for relative coordinates. **Caveat on
  macOS Tahoe:** AX-reported window origin includes the drop-shadow inset
  (~40 px), so clicks land below intent. Prefer screen-absolute coords
  from `testanyware screenshot` when precision matters.

### `testanyware input mouse-down`

**Synopsis:** Press a mouse button without releasing.

```
testanyware input mouse-down <x> <y> [--button <button>] [--window <window>]
```

### `testanyware input mouse-up`

**Synopsis:** Release a mouse button.

```
testanyware input mouse-up <x> <y> [--button <button>] [--window <window>]
```

### `testanyware input move`

**Synopsis:** Move the mouse cursor.

```
testanyware input move <x> <y> [--window <window>]
```

### `testanyware input scroll`

**Synopsis:** Scroll at coordinates.

```
testanyware input scroll <x> <y> [--dx <dx>] [--dy <dy>] [--window <window>]
```

- `--dx <dx>` — Horizontal scroll amount (negative = left). Default: `0`.
- `--dy <dy>` — Vertical scroll amount (negative = up). Default: `0`.

### `testanyware input drag`

**Synopsis:** Drag from one point to another, with interpolation.

```
testanyware input drag <from-x> <from-y> <to-x> <to-y> [--button <button>] [--steps <steps>] [--window <window>]
```

- `-s, --steps <steps>` — Number of interpolation steps. Default: `10`.

---

## `testanyware exec`

**Synopsis:** Execute a command on the VM via agent; captures stdout,
stderr, and exit code.

```
testanyware exec <command> [--detach]
```

- `<command>` — Command string.
- `--detach` — Launch detached; return immediately without waiting.

**Example:**
```bash
testanyware exec "uname -a"
testanyware exec "/usr/bin/open -a Calculator" --detach
```

---

## `testanyware upload`

**Synopsis:** Upload a file to the VM via agent.

```
testanyware upload <local-path> <remote-path>
```

---

## `testanyware download`

**Synopsis:** Download a file from the VM via agent.

```
testanyware download <remote-path> <local-path>
```

---

## `testanyware record`

**Synopsis:** Record the VNC screen to a video file (H.264/HEVC via
`AVAssetWriter`).

```
testanyware record [-o <output>] [--fps <fps>] [--duration <duration>] [--region <region>]
```

- `-o, --output <output>` — Output file path. Default: `recording.mp4`.
- `--fps <fps>` — Frames per second. Default: `30`.
- `--duration <duration>` — Duration in seconds (0 = use max 300s).
  Default: `0`.
- `--region <region>` — Crop region as `x,y,width,height`.

**Example:**
```bash
testanyware record -o test-run.mp4 --fps 30 --duration 10
```

---

## `testanyware find-text`

**Synopsis:** Find text on screen using OCR. Uses Apple Vision on macOS
hosts and the EasyOCR daemon on Linux/Windows.

```
testanyware find-text [<text>] [--timeout <timeout>]
```

- `<text>` — Text to search for (case-insensitive substring match). Omit
  to return all recognized text.
- `--timeout <timeout>` — Wait up to N seconds for the text to appear.

**Example:**
```bash
testanyware find-text "Terminal"                 # one-shot
testanyware find-text "Loading" --timeout 30     # poll
testanyware find-text                            # dump all OCR
```

Output is JSON with per-match `text`, `confidence`, and `bounds`.

---

## `testanyware agent`

Container for in-VM accessibility operations. Sub-sub-commands below.

Every `agent` command that targets an element accepts the standard
**element query** set:

- `--role <role>` — Element role (see `docs/architecture/agent-protocol.md`
  for the unified role list).
- `--label <label>` — Element label filter.
- `--id <id>` — Element id (platform-native identifier).
- `--index <index>` — Element index (0-based) when multiple match.
- `--window <window>` — Window title or app name filter.

### `testanyware agent health`

**Synopsis:** Check agent health. Prints `ok` on success.

```
testanyware agent health
```

### `testanyware agent snapshot`

**Synopsis:** Capture an accessibility element tree snapshot.

```
testanyware agent snapshot [--mode <mode>] [--window <window>] [--role <role>] [--label <label>] [--depth <depth>] [--json]
```

- `--mode <mode>` — `full` (all roles) or `interactive` (buttons, inputs,
  text only).
- `--depth <depth>` — Maximum tree depth.
- `--json` — Output raw JSON instead of formatted text.

### `testanyware agent inspect`

**Synopsis:** Inspect a single element in detail (font, color, bounds).

```
testanyware agent inspect [element-query] [--json]
```

### `testanyware agent press`

**Synopsis:** Press (activate) an element — the semantic "default
action" for its role.

```
testanyware agent press [element-query]
```

### `testanyware agent set-value`

**Synopsis:** Set the value of an element (text field, slider, etc.).

```
testanyware agent set-value [element-query] --value <value>
```

### `testanyware agent focus`

**Synopsis:** Focus an element.

```
testanyware agent focus [element-query]
```

### `testanyware agent show-menu`

**Synopsis:** Show the context menu of an element.

```
testanyware agent show-menu [element-query]
```

### `testanyware agent windows`

**Synopsis:** List all windows visible to the accessibility layer.

```
testanyware agent windows
```

### `testanyware agent window-focus`

**Synopsis:** Focus (raise) a window.

```
testanyware agent window-focus --window <window>
```

### `testanyware agent window-resize`

**Synopsis:** Resize a window.

```
testanyware agent window-resize --window <window> --width <width> --height <height>
```

### `testanyware agent window-move`

**Synopsis:** Move a window.

```
testanyware agent window-move --window <window> --x <x> --y <y>
```

### `testanyware agent window-close`

**Synopsis:** Close a window.

```
testanyware agent window-close --window <window>
```

### `testanyware agent window-minimize`

**Synopsis:** Minimize a window.

```
testanyware agent window-minimize --window <window>
```

### `testanyware agent wait`

**Synopsis:** Wait for accessibility to be ready (agent reachable + AX
initialized). Useful as the first step of a script after a fresh boot.

```
testanyware agent wait [--window <window>] [--timeout <timeout>]
```

---

## `testanyware vm`

Container for VM lifecycle operations. Sub-sub-commands below.

### `testanyware vm start`

**Synopsis:** Start a VM and register its spec under
`$XDG_STATE_HOME/testanyware/vms/<id>.json`. Prints the instance id on
stdout.

```
testanyware vm start [--platform <platform>] [--base <base>] [--id <id>] [--display <display>] [--viewer] [--no-ssh]
```

- `--platform <platform>` — `macos`, `linux`, or `windows`. Default: `macos`.
- `--base <base>` — Base image to clone from. Default: platform-specific
  golden image.
- `--id <id>` — VM instance id. Default: `testanyware-<hex8>`.
- `--display <display>` — Display resolution (e.g. `1920x1080`).
- `--viewer` — Open a VNC viewer after boot.
- `--no-ssh` — Accepted for backward compatibility; ignored (deprecated).

**Example:**
```bash
vmid=$(testanyware vm start --viewer)
export TESTANYWARE_VM_ID="$vmid"
```

### `testanyware vm stop`

**Synopsis:** Stop a VM and remove its spec file.

```
testanyware vm stop [<id>]
```

- `<id>` — VM instance id. Falls back to `TESTANYWARE_VM_ID`.

### `testanyware vm list`

**Synopsis:** List golden images and running clones across tart and QEMU
backends.

```
testanyware vm list
```

### `testanyware vm delete`

**Synopsis:** Delete a golden image by name. Auto-detects tart vs QEMU
backend.

```
testanyware vm delete <name> [--force]
```

- `<name>` — Golden image name (run `testanyware vm list` to see available).
- `--force` — Delete even if running clones appear to depend on the image.
