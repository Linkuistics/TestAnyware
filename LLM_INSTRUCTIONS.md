# TestAnyware — LLM Instructions

## Architecture

Two independent channels to each VM:

| Channel | Transport | Purpose | Endpoint |
|---------|-----------|---------|----------|
| **VNC** | RFB protocol | Screenshots, keyboard/mouse input, video recording | `--vnc host:port` |
| **Agent** | HTTP/1.1 JSON on port 8648 | Accessibility tree, UI actions, exec, file transfer, shutdown | `--agent host:port` |

Each VM runs an agent HTTP server on port 8648. The host CLI (`testanyware`) talks to both channels. VNC is the visual/input channel; agent is the semantic/exec channel.

**Platform agents:** macOS (Swift/Hummingbird), Windows (C#/ASP.NET), Linux (Python/http.server). All expose identical HTTP endpoints.

**Host-side vision:** A Python `uv` workspace at `vision/` turns raw screenshots into structured UI data (windows, chrome, elements, icons, drawing primitives, menus, OCR, state). See [Vision Pipeline](#vision-pipeline) below.

## Quick Reference

### Start a VM

```bash
vmid=$(provisioner/scripts/vm-start.sh)                                          # macOS (default)
vmid=$(provisioner/scripts/vm-start.sh --platform linux)                         # Linux
vmid=$(provisioner/scripts/vm-start.sh --platform windows)                       # Windows
vmid=$(provisioner/scripts/vm-start.sh --platform windows --display 1920x1080)   # Windows at 1920x1080
vmid=$(provisioner/scripts/vm-start.sh --id my-vm)                               # reuse a known id
```

`vm-start.sh` runs as a normal subprocess (no `source` needed). It prints the VM instance id on stdout and writes (paths follow XDG; defaults shown):

- `~/.local/state/testanyware/vms/<id>.json` — public connect spec (VNC, agent, platform, ssh). Mode 600.
- `~/.local/state/testanyware/vms/<id>.meta.json` — private lifecycle metadata for `vm-stop.sh` (PID, tool, clone dir).

Override with `$XDG_STATE_HOME`. QEMU golden images and clones live under `$XDG_DATA_HOME/testanyware/` (default `~/.local/share/testanyware/`).

Pass the id to every subsequent CLI call via `--vm <id>`, or export it once:

```bash
export TESTANYWARE_VM_ID="$vmid"
```

**Client automation should persist the id to a file** local to the operation
so that later steps (even in a fresh process) can find the VM to use and
stop it:

```bash
vmid=$(provisioner/scripts/vm-start.sh)
printf '%s\n' "$vmid" > .testanyware-vmid        # commit-ignored handle
testanyware screenshot --vm "$(cat .testanyware-vmid)" -o out.png
provisioner/scripts/vm-stop.sh "$(cat .testanyware-vmid)"
rm .testanyware-vmid
```

**Resolution order** (highest priority first): `--connect <path>` → `--vm <id>` → `--vnc`/`--agent`/`--platform` flags → `TESTANYWARE_VM_ID` → `TESTANYWARE_VNC`/`TESTANYWARE_AGENT`/etc. → error.

### Multi-VM Setup

Run multiple VMs simultaneously — each gets its own id and its own spec file:

```bash
macid=$(provisioner/scripts/vm-start.sh)                    # macOS
linid=$(provisioner/scripts/vm-start.sh --platform linux)   # Linux
winid=$(provisioner/scripts/vm-start.sh --platform windows) # Windows

# Interact with each independently via --vm:
testanyware exec --vm "$macid" "uname -a"
testanyware exec --vm "$linid" "uname -a"
testanyware exec --vm "$winid" "systeminfo | findstr /B /C:\"OS Name\""

# If you need a VM's agent/VNC endpoint directly, read its spec file:
MAC_AGENT=$(python3 -c "import json; s=json.load(open('${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/$macid.json')); a=s['agent']; print(f\"{a['host']}:{a['port']}\")")
LIN_AGENT=$(python3 -c "import json; s=json.load(open('${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/$linid.json')); a=s['agent']; print(f\"{a['host']}:{a['port']}\")")

# VMs on the same tart network can reach each other via IP:
MAC_IP=${MAC_AGENT%:*}
testanyware exec --vm "$linid" "curl -sf http://$MAC_IP:8648/health"
```

### Stop VMs

```bash
provisioner/scripts/vm-stop.sh "$vmid"                    # positional id
TESTANYWARE_VM_ID=$vmid provisioner/scripts/vm-stop.sh    # env form
```

An id is required — there's no auto-discovery, because QEMU has no central registry and a tart-only fallback would be inconsistent across runners.

### VM lifecycle via `testanyware vm`

The bash scripts are thin wrappers around `testanyware vm` subcommands. Use either form:

- `testanyware vm start [--platform macos|linux|windows] [--id <id>] [--display WxH] [--viewer]` — start a VM; prints the id on stdout.
- `testanyware vm stop <id>` — stop a VM and remove its spec + meta sidecar.
- `testanyware vm list` — list golden images and running clones (tart + QEMU).
- `testanyware vm delete <name> [--force]` — delete a golden image by name; auto-detects tart vs QEMU backend. Refuses if running clones appear to depend on the image unless `--force` is passed.

`testanyware vm start --viewer` and `testanyware vm stop` open and close a VNC viewer via AppleScript; the first invocation will prompt for Automation permission (System Settings → Privacy & Security → Automation → testanyware → System Events).

## Agent Commands

### Exec & File Transfer

```bash
testanyware exec --agent host:port "command"                  # run shell command
testanyware upload --agent host:port localpath remotepath      # upload file
testanyware download --agent host:port remotepath localpath    # download file
```

### Accessibility

```bash
testanyware agent health --agent host:port                    # check agent + accessibility status
testanyware agent windows --agent host:port                   # list all windows
testanyware agent snapshot --agent host:port [options]        # accessibility tree snapshot
testanyware agent inspect --agent host:port [query]           # detailed element properties + bounds
```

Snapshot options: `--mode interact|layout|full`, `--window FILTER`, `--role ROLE`, `--label TEXT`, `--depth N`

### UI Actions

```bash
testanyware agent press --agent host:port [query]             # press/click element
testanyware agent set-value --agent host:port [query] --value TEXT  # set text/slider value
testanyware agent focus --agent host:port [query]             # focus element
testanyware agent show-menu --agent host:port [query]         # open context menu
```

Query parameters: `--role ROLE`, `--label TEXT`, `--window FILTER`, `--id ID`, `--index N`

### Window Management

```bash
testanyware agent window-focus --agent host:port --window FILTER
testanyware agent window-resize --agent host:port --window FILTER --width W --height H
testanyware agent window-move --agent host:port --window FILTER --x X --y Y
testanyware agent window-close --agent host:port --window FILTER
testanyware agent window-minimize --agent host:port --window FILTER
testanyware agent wait --agent host:port [--timeout SECONDS]
```

## VNC Commands

### Screenshots & Display

```bash
testanyware screen-size --vnc host:port                       # "1920x1080"
testanyware screenshot --vnc host:port -o file.png            # full screen
testanyware screenshot --vnc host:port --region x,y,w,h -o file.png  # cropped
```

### Keyboard

```bash
testanyware input key --vnc host:port KEYNAME [--modifiers mod1,mod2]
testanyware input key-down --vnc host:port KEYNAME            # hold key
testanyware input key-up --vnc host:port KEYNAME              # release key
testanyware input type --vnc host:port "text to type"         # type string
```

Keys: `a`-`z` `0`-`9` `return` `tab` `escape` `space` `delete` `backspace` `forwarddelete` `up` `down` `left` `right` `home` `end` `pageup` `pagedown` `f1`-`f19`
Modifiers: `cmd` `alt` `shift` `ctrl` — mapped per `--platform`

### Mouse

```bash
testanyware input click --vnc host:port X Y [--button right] [--count 2]
testanyware input mouse-down --vnc host:port X Y [--button left|right|middle]
testanyware input mouse-up --vnc host:port X Y
testanyware input move --vnc host:port X Y
testanyware input scroll --vnc host:port X Y --dy -3          # negative = up
testanyware input drag --vnc host:port fromX fromY toX toY
```

### OCR

```bash
testanyware find-text --vnc host:port "search text"           # find text, returns JSON with coords
testanyware find-text --vnc host:port "Loading" --timeout 30  # poll until found
testanyware find-text --vnc host:port                         # all text on screen
```

Returns: `[{"text":"Terminal","x":248,"y":91,"width":55,"height":12,"confidence":0.95}]`
Click center: `x + width/2`, `y + height/2`

### Video Recording

```bash
testanyware record --vnc host:port -o out.mp4 --fps 30 --duration 10
```

## Connection Spec JSON

The spec file `vm-start.sh` writes (and that `--vm <id>` / `--connect <path>` consume):

```json
{"vnc":      {"host": "localhost", "port": 5900, "password": "secret"},
 "agent":    {"host": "192.168.64.100", "port": 8648},
 "platform": "macos",
 "ssh":      "admin@192.168.64.100"}
```

- `vnc` always present; `vnc.password` present on tart (macOS/Linux) and QEMU (Windows).
- `agent` present when the agent reached health during startup.
- `platform` always present: `macos` | `linux` | `windows`.
- `ssh` is a debug convenience (tart only), not consumed by the CLI.

Pass via `--connect spec.json` for an explicit file, or `--vm <id>` to resolve automatically from `${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/<id>.json`.

## Environment Variables

| Variable | Example | Purpose |
|----------|---------|---------|
| `TESTANYWARE_VM_ID` | `testanyware-a3f7b2c1` | Resolves to the per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json` |
| `TESTANYWARE_AGENT` | `192.168.64.100:8648` | Agent HTTP endpoint (overrides the spec file's agent) |
| `TESTANYWARE_VNC` | `127.0.0.1:59948` | VNC endpoint (ad-hoc; no spec file needed) |
| `TESTANYWARE_VNC_PASSWORD` | `syrup-rotate-nasty` | VNC password used with `TESTANYWARE_VNC` |
| `TESTANYWARE_PLATFORM` | `macos` | Target platform (`macos`, `linux`, `windows`) |

## Workflow Patterns

All examples below assume `TESTANYWARE_VM_ID` is set (or that you pass `--vm <id>` on every command).

### Discover-then-act (preferred)

```bash
# 1. See what's on screen semantically
testanyware agent snapshot --mode interact

# 2. Act on elements by role/label (not pixel coordinates)
testanyware agent press --role button --label "Save"

# 3. Verify result
testanyware agent snapshot --mode interact
```

### Visual verification

```bash
# Screenshot + OCR for visual properties (colors, layout, rendered text)
testanyware screenshot -o screen.png
testanyware find-text "Expected text"
```

### Cross-VM communication

```bash
# Start a server in VM1, connect from VM2
testanyware exec --vm "$vm1id" "python3 -m http.server 9000 &"
VM1_IP=$(python3 -c "import json; print(json.load(open('${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/$vm1id.json'))['agent']['host'])")
testanyware exec --vm "$vm2id" "curl -sf http://$VM1_IP:9000/"
```

### Install and test software

```bash
# macOS
testanyware exec "brew install jq"

# Linux
testanyware exec "sudo apt-get install -y jq"

# Windows
testanyware exec "choco install jq -y"
```

## Golden Images

Pre-built VM images with clean desktops, agent pre-installed, auto-login enabled.

| Image | Hypervisor | User | Agent Autostart | Package Manager |
|-------|-----------|------|-----------------|-----------------|
| `testanyware-golden-macos-tahoe` | tart | admin | LaunchAgent (`com.linkuistics.testanyware-agent`) | Homebrew |
| `testanyware-golden-linux-24.04` | tart | admin | systemd user service (`testanyware-agent.service`) | apt |
| `testanyware-golden-windows-11` | QEMU | admin | Task Scheduler (`TestAnywareAgent`) | Chocolatey |

All images: solid gray wallpaper, no notifications/widgets, accessibility enabled, agent on port 8648.

### Create golden images (one-time)

```bash
provisioner/scripts/vm-create-golden-macos.sh
provisioner/scripts/vm-create-golden-linux.sh
provisioner/scripts/vm-create-golden-windows.sh --iso ~/Downloads/Win11_ARM64.iso
```

## Vision Pipeline

A Python `uv` workspace at `vision/` decomposes screenshots into structured
UI data through sequential stages. Each stage consumes the previous stage's
output (plus the raw image when needed) and emits typed artifacts.

| Stage | Input | Output |
|---|---|---|
| window-detection | Screenshot | Per-window bounding boxes |
| chrome-detection | Screenshot + windows | OS chrome regions (title bars, toolbars, status bars) |
| element-detection | Window crop | Per-element detections (bounds + role hint) |
| icon-classification | Element crops (buttons) | Semantic icon labels — **shape-heuristic fallback; learned model not yet trained** |
| drawing-primitives | Element crops | Color, border, shadow, font |
| menu-detection | Screenshot | Menu bar + contextual menus |
| ocr | Screenshot | Text regions (via Apple Vision framework on macOS host) |
| state-detection | Elements + OCR | Enabled/disabled/checked inference |

**Icon classification note:** the learned icon classifier has not been
trained yet. The pipeline currently uses a shape-heuristic classifier as
a placeholder. Treat its labels as low-confidence hints — prefer
accessibility role/label from the in-VM agent when possible.

### Running the pipeline

```bash
cd vision
uv sync
uv run python -m pipeline.orchestrator --image path/to/screen.png
```

### Running tests

Always pass `--import-mode=importlib` so pytest does not collide on
duplicate `tests/` package names across workspace members:

```bash
cd vision
uv run pytest --import-mode=importlib                # unit + vision
uv run pytest --import-mode=importlib -m integration # needs live VMs
uv run pytest --import-mode=importlib -m slow
uv run pytest --import-mode=importlib -m vision
```

### Calling from the CLI

The `testanyware` CLI does not yet shell out to the vision pipeline
directly. Until then, callers invoke the pipeline as a Python subprocess
or via its FastAPI server (see `docs/architecture/vision-pipeline.md`).

## Tips

- **Prefer accessibility over coordinates**: `--role button --label "Save"` is more robust than clicking at pixel (x, y)
- **Use `find-text` for visual elements**: OCR finds rendered text that accessibility can't see (images, canvas, custom-drawn UI)
- **Sleep after input**: VMs need time to process events — `sleep 1` after keyboard/mouse, `sleep 5` after launching apps
- **Multi-VM networking**: tart VMs share a network bridge; QEMU uses NAT with port forwarding. tart VMs can reach each other by IP; QEMU VMs need explicit port forwards
- **Display resolution**: `--display 1920x1080` sets VM display size. Works for all platforms (tart uses `tart set --display`, QEMU uses `virtio-gpu-pci` xres/yres)
- **Platform modifiers**: `--platform macos` maps `cmd` to macOS Command key; `--platform windows` maps `cmd` to Ctrl
- **Connection caching**: The first `testanyware` command auto-starts a background VNC server process. Subsequent commands reuse it. Idle timeout: 5 minutes.
- **Agent is the primary channel**: Use agent exec instead of SSH. VNC is for visual verification and keyboard/mouse when accessibility can't target an element.
- **Windows agent build**: when rebuilding the Windows agent from the macOS host, use `dotnet build -r win-arm64 --no-self-contained` (the autounattend image provides the .NET runtime in-VM).
- **Monorepo layout**: the Swift CLI lives directly at `cli/` (no `cli/macos/` subdir yet — Rust port pending). The macOS agent under `agents/macos/` is self-contained and ships its own copy of `TestAnywareAgentProtocol`; the CLI-side copy under `cli/Sources/TestAnywareAgentProtocol/` is independent and kept in sync by hand.
