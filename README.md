# TestAnyware

> AI-driven GUI testing across virtual machines. Cross-platform guest
> support (macOS, Linux, Windows) via VNC + in-VM HTTP agents. Python
> vision pipeline for structured screen understanding.

**For LLM consumers:** see [LLM_INSTRUCTIONS.md](LLM_INSTRUCTIONS.md).
**For contributors:** see [`docs/`](docs/).
**Design history:** see [`0-docs/designs/`](0-docs/designs/).

## What It Does

**VNC capture and input:**
- Connect to any VNC server, capture screenshots (full or cropped), record video
- Query display dimensions
- Send keyboard events with platform-aware modifier mapping (Cmd/Alt/Ctrl work correctly on macOS, Windows, and Linux VMs)
- Individual key-down/key-up and mouse-down/mouse-up for fine-grained control
- Send mouse events: click, double-click, right-click, drag, scroll
- Type text with automatic handling of uppercase and shifted symbols

**In-VM agent communication** (HTTP on port 8648):
- Accessibility tree access — query window lists, element snapshots, element inspection
- Semantic UI actions — press buttons, activate controls by role and label
- Command execution with stdout/stderr/exit code capture
- File upload and download

**Streaming video capture:**
- Record VNC framebuffer to MP4 using AVAssetWriter
- Configurable resolution, frame rate, and codec (H.264/HEVC)

**Vision pipeline** (Python):
- Structured screen understanding: window detection, chrome detection,
  element detection, icon classification, drawing-primitive extraction,
  menu detection, OCR, state detection
- Shape-heuristic icon classifier as a pre-model fallback (the learned
  classifier is not yet trained)

## CLI

```bash
# Display info
testanyware screen-size --vnc localhost:5901                              # prints "1920x1080"

# Screenshot
testanyware screenshot --vnc localhost:5901 -o screen.png
testanyware screenshot --vnc localhost:5901 --region 0,0,800,600 -o cropped.png

# Keyboard — press and release
testanyware input key --vnc localhost:5901 return
testanyware input key --vnc localhost:5901 a --modifiers cmd
testanyware input key --vnc localhost:5901 z --modifiers cmd,shift

# Keyboard — individual down/up
testanyware input key-down --vnc localhost:5901 shift
testanyware input key-up --vnc localhost:5901 shift

# Text entry
testanyware input type --vnc localhost:5901 "Hello World!"

# Mouse — click
testanyware input click --vnc localhost:5901 500 400
testanyware input click --vnc localhost:5901 500 400 --button right
testanyware input click --vnc localhost:5901 500 400 --count 2

# Mouse — individual down/up
testanyware input mouse-down --vnc localhost:5901 100 200
testanyware input mouse-down --vnc localhost:5901 100 200 --button right
testanyware input mouse-up --vnc localhost:5901 100 200

# Mouse — move, scroll, drag
testanyware input move --vnc localhost:5901 100 200
testanyware input scroll --vnc localhost:5901 500 400 --dy -3
testanyware input drag --vnc localhost:5901 100 100 400 400

# OCR — find text on screen (captures VNC screenshot + runs Vision OCR locally)
testanyware find-text --vnc localhost:5901 "Terminal"             # find text, return JSON with coords
testanyware find-text --vnc localhost:5901 "Loading" --timeout 30 # poll until text appears (30s max)
testanyware find-text --vnc localhost:5901                        # return all recognized text

# Agent — exec and file transfer
testanyware exec --agent localhost:8648 "uname -a"
testanyware upload --agent localhost:8648 local.txt /tmp/remote.txt
testanyware download --agent localhost:8648 /tmp/remote.txt local.txt

# Agent — accessibility
testanyware agent health --agent localhost:8648
testanyware agent windows --agent localhost:8648
testanyware agent snapshot --agent localhost:8648 --mode interact --window "Settings"
testanyware agent snapshot --vm "$vmid" --open-menu File          # open the File menu, then snapshot it (macOS menu items are lazy)
testanyware agent inspect --agent localhost:8648 --role button --label "Save"
testanyware agent press --agent localhost:8648 --role button --label "Save"

# Video recording
testanyware record --vnc localhost:5901 -o recording.mp4 --fps 30 --duration 10

# VM lifecycle (also available as provisioner/scripts/vm-*.sh thin wrappers)
testanyware vm start --platform macos --display 1920x1080 --viewer      # prints the id on stdout
testanyware vm stop <vm-id>
testanyware vm list
testanyware vm delete <golden-image-name> [--force]
```

All commands accept `--connect spec.json` for connection details from a JSON
file, `--vm <id>` for a VM started via `vm-start.sh`, or individual `--vnc`,
`--agent`, and `--platform` flags.

### Key names

Letters: `a`-`z` | Digits: `0`-`9` | Special: `return` `enter` `tab` `escape` `esc` `space` `delete` `backspace` `forwarddelete` | Arrows: `up` `down` `left` `right` | Navigation: `home` `end` `pageup` `pagedown` | Function: `f1`-`f19`

### Modifier names

`cmd` `command` `alt` `option` `shift` `ctrl` `control` (mapped correctly per `--platform`)

### Mouse buttons

`left` `right` `middle` `center`

### Display resolution

VNC cannot change the display resolution. Use `--display WxH` when starting a VM:

```bash
provisioner/scripts/vm-start.sh --display 1920x1080                      # macOS & Linux
provisioner/scripts/vm-start.sh --platform windows --display 1920x1080   # Windows
```

## Library

```swift
import TestAnywareDriver

// Connect
let capture = VNCCapture(host: "localhost", port: 5901, password: "secret")
try await capture.connect()

// Screen size
let size = await capture.screenSize() // CGSize?

// Screenshot
let image = try await capture.captureImage()
let png = try await capture.screenshot()

// Input
try await capture.withConnection { conn in
    VNCInput.typeText("Hello", connection: conn)
    try VNCInput.pressKey("return", platform: .macos, connection: conn)
    try VNCInput.click(x: 500, y: 400, connection: conn)

    // Fine-grained control
    try VNCInput.keyDown("shift", platform: .macos, connection: conn)
    try VNCInput.keyUp("shift", platform: .macos, connection: conn)
    try VNCInput.mouseDown(x: 100, y: 200, button: "left", connection: conn)
    try VNCInput.mouseUp(x: 300, y: 400, button: "left", connection: conn)
}

// Agent communication
let agent = AgentTCPClient(host: "192.168.64.100", port: 8648)
let health = try await agent.health()
let snapshot = try await agent.snapshot(mode: "interact", window: "Settings")
let execResult = try await agent.exec(command: "uname -a")

// Video recording
let recorder = StreamingCapture()
try await recorder.start(outputPath: "out.mp4", config: .init(width: 1920, height: 1080))
try await recorder.appendFrame(image)
try await recorder.stop()
```

## Architecture

Two-channel, platform-agnostic design:

- **VNC channel** (RFB protocol): Raw framebuffer capture and input. Handles
  screenshots (full/cropped), video recording to MP4, keyboard (with
  platform-aware modifier mapping), and mouse events.
- **Agent channel** (HTTP/1.1 JSON on port 8648): Per-platform accessibility
  server. Exposes element trees, semantic actions (press, set-value, focus),
  exec with captured stdout/stderr, file transfer, and window management.

**Platform agents:** Three independent implementations using native
accessibility APIs:

- macOS: Swift/Hummingbird HTTP server, uses ApplicationServices/AXUIElement
- Linux: Python/http.server, uses AT-SPI2 (with xdotool fallback for GTK4
  coordinate bugs)
- Windows: C#/ASP.NET, uses UI Automation (UIA) via FlaUI

**Host side (Swift):** `TestAnywareDriver` wraps the RFB protocol and agent
HTTP client. `TestAnywareAgentProtocol` defines the shared wire-format types
used by both the CLI and the macOS agent (note: each side currently ships
its own self-contained copy of this module — see §Key Directories). CLI is
built on swift-argument-parser.

**Vision pipeline (Python):** A `uv` workspace at `vision/` decomposes
screenshots into structured UI data through sequential stages (window
detection → chrome → element → icon classification → drawing primitives →
menu → OCR → state). Uses pytest with `--import-mode=importlib` to avoid
duplicate-module collisions across the workspace members.

## Tech Stack & Languages

| Component | Language | Why | Platform |
|---|---|---|---|
| CLI / host library | Swift 6 | Native performance, async/await, AVAssetWriter for MP4 | macOS 14+ host only |
| macOS agent | Swift | Native AX APIs, Hummingbird for HTTP | In-VM only |
| Linux agent | Python 3.12+ | AT-SPI2 bindings, http.server minimal footprint | In-VM only |
| Windows agent | C# 9 | UIA/FlaUI, ASP.NET Core, ships as standalone .NET 9 app | In-VM only |
| Vision pipeline | Python 3.11+ | Pillow, pytest, uv workspace | Host-side analysis |
| Provisioner | Bash + autounattend XML | VM lifecycle, golden-image creation | macOS 14+ host only |

**Notable packages:**

- `royalvnc` (vendored): RFB protocol implementation
- `hummingbird`: HTTP framework for the macOS agent
- `swift-argument-parser`: CLI argument handling
- FlaUI (C#): UI Automation wrapper for Windows
- `uv`: Python workspace / dependency manager for the vision pipeline

## Install (Homebrew)

```bash
brew install Linkuistics/taps/testanyware
```

That installs the `testanyware` host CLI on PATH and bundles the in-VM
agents (macOS Swift binary, Windows .NET 9 self-contained `.exe`, Linux
Python source) and the golden-image scripts under
`$(brew --prefix testanyware)/share/testanyware/`. The
`vm-create-golden-{macos,linux,windows}.sh` scripts read agent
artifacts from that location automatically — there is nothing to build
on the host.

To create a golden image, invoke the bundled scripts directly:

```bash
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-macos.sh"
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-linux.sh"
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-windows.sh" --iso ~/Downloads/Win11_ARM64.iso
```

`brew upgrade testanyware` updates the CLI and the bundled agents
together (they share the wire protocol; never split). Rebuild your
goldens after upgrading the formula so the in-VM agent matches the
host CLI.

## Building from Source

For contributors only. End users should install via Homebrew (above).
Building from source overrides the brew-bundled agents via two env
vars consumed by the golden scripts:

| Override | Used for |
|----------|----------|
| `TESTANYWARE_CLI_BIN_OVERRIDE=/abs/path/to/testanyware` | macOS golden's recovery-mode VNC driver (host CLI) |
| `TESTANYWARE_AGENT_BIN_OVERRIDE=/abs/path/to/{testanyware-agent,testanyware-agent.exe,agents/linux}` | per-platform agent payload |

If neither is set, the scripts resolve `command -v testanyware` and
`brew --prefix testanyware`/`share/testanyware/agents/<platform>/...`.

**CLI (Swift, macOS host):**

```bash
cd cli
swift build                                # build executable
swift build -c release                     # release binary
```

The `cli/` package is currently flat (no `cli/macos/` subdirectory). A
cross-platform Rust port is planned; until then the Swift package lives
directly at `cli/`.

**macOS agent:**

```bash
cd agents/macos
swift build
swift build -c release
```

The macOS agent is self-contained: it carries its own copy of
`TestAnywareAgentProtocol` under `agents/macos/Sources/`. The CLI has an
independent copy at `cli/Sources/TestAnywareAgentProtocol/`. The two
copies must be kept in sync by hand until the shared package is unified.

**Windows agent (built on macOS host, cross-compiled for Windows ARM64):**

```bash
cd agents/windows
dotnet build -r win-arm64 --no-self-contained
dotnet publish -c Release -r win-arm64 --no-self-contained
```

**Linux agent:**

```bash
cd agents/linux
# No explicit build; ships as Python source with testanyware-agent wrapper script
```

**Vision pipeline:**

```bash
cd vision
uv sync                                              # install workspace
uv run pytest --import-mode=importlib                # unit + vision (skips integration/slow)
uv run pytest --import-mode=importlib -m integration # full pipeline tests (needs live VMs)
uv run pytest --import-mode=importlib -m slow        # slow tests
uv run pytest --import-mode=importlib -m vision      # vision accuracy tests only
uv run ruff check --fix                              # format & lint
```

`--import-mode=importlib` avoids duplicate-module collisions across the
multiple workspace members that define `tests/` packages.

## Integration Testing

Tests run against a real VM via [tart](https://tart.run) (macOS, Linux) or
QEMU (Windows). Golden VM images provide clean environments with the in-VM
agent pre-installed.

### First-time setup

Create golden images (one-time, ~10 minutes each):

```bash
provisioner/scripts/vm-create-golden-macos.sh     # macOS (tart)
provisioner/scripts/vm-create-golden-linux.sh     # Linux (tart)
```

For Windows, first download the Windows 11 ARM64 ISO from
[Microsoft](https://www.microsoft.com/en-us/software-download/windows11arm64),
then pass it to the script:

```bash
provisioner/scripts/vm-create-golden-windows.sh --iso ~/Downloads/Win11_ARM64.iso
```

The ISO is cached after first use — subsequent runs don't need `--iso`.
The Windows installation is fully automated via `autounattend.xml`
(typical time: 20-40 minutes).

### Running tests

```bash
# Start a VM (captures the generated id), run tests, stop when done
vmid=$(provisioner/scripts/vm-start.sh --viewer)                          # macOS (default)
vmid=$(provisioner/scripts/vm-start.sh --platform linux --viewer)         # Linux
vmid=$(provisioner/scripts/vm-start.sh --platform windows --viewer)       # Windows
export TESTANYWARE_VM_ID="$vmid"   # the CLI auto-resolves --vm from this
swift test --package-path cli --filter IntegrationTests
provisioner/scripts/vm-stop.sh "$vmid"
```

### Unit tests only (no VM needed)

```bash
swift test --package-path cli
```

## Scripts

`provisioner/scripts/vm-{start,stop,list,delete}.sh` are thin wrappers
around `testanyware vm {start,stop,list,delete}`. You can call either
form; the CLI is the source of truth.

| Script | How to run | What it does |
|--------|-----------|--------------|
| `provisioner/scripts/vm-create-golden-macos.sh` | `./provisioner/scripts/vm-create-golden-macos.sh` | Create macOS golden VM image (tart) with agent + Xcode + Homebrew |
| `provisioner/scripts/vm-create-golden-linux.sh` | `./provisioner/scripts/vm-create-golden-linux.sh` | Create Linux golden VM image (tart) with agent + dev tools |
| `provisioner/scripts/vm-create-golden-windows.sh` | `./provisioner/scripts/vm-create-golden-windows.sh --iso <path>` | Create Windows golden VM image (QEMU) with agent + Chocolatey; requires downloaded ISO on first run |
| `provisioner/scripts/vm-start.sh` | `vmid=$(provisioner/scripts/vm-start.sh)` | Start VM, print instance id on stdout, write `$XDG_STATE_HOME/testanyware/vms/<id>.json` |
| `provisioner/scripts/vm-stop.sh` | `provisioner/scripts/vm-stop.sh "$vmid"` | Stop VM and delete its spec files (id is required; `TESTANYWARE_VM_ID` works too) |
| `provisioner/scripts/vm-list.sh` | `provisioner/scripts/vm-list.sh` | List golden images and running clones (tart + QEMU) |
| `provisioner/scripts/vm-delete.sh` | `provisioner/scripts/vm-delete.sh <name> [--force]` | Delete a golden image by name; auto-detects tart vs QEMU backend |

### Environment variables

| Variable | Example | Description |
|----------|---------|-------------|
| `TESTANYWARE_VM_ID` | `testanyware-a3f7b2c1` | VM instance id; CLI resolves it to `$XDG_STATE_HOME/testanyware/vms/<id>.json` |
| `TESTANYWARE_AGENT` | `192.168.64.100:8648` | Agent HTTP endpoint (overrides the spec file's agent) |
| `TESTANYWARE_VNC` | `127.0.0.1:59948` | VNC endpoint (ad-hoc; no spec file needed) |
| `TESTANYWARE_VNC_PASSWORD` | `syrup-rotate-nasty` | VNC password used with `TESTANYWARE_VNC` |
| `TESTANYWARE_PLATFORM` | `macos` | Target platform (`macos`, `linux`, `windows`) |

### Connection resolution

Every subcommand resolves connection details through this chain
(highest priority first):

1. `--connect <path>` — explicit spec file
2. `--vm <id>` — per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json`
3. `--vnc` / `--agent` / `--platform` — explicit flags
4. `TESTANYWARE_VM_ID` — resolves to the per-VM spec like `--vm`
5. `TESTANYWARE_VNC` / `TESTANYWARE_VNC_PASSWORD` / `TESTANYWARE_AGENT` /
   `TESTANYWARE_PLATFORM` — direct env vars
6. Error

Typical workflow:

```bash
vmid=$(provisioner/scripts/vm-start.sh)
export TESTANYWARE_VM_ID="$vmid"    # optional — lets you omit --vm
testanyware screenshot -o screen.png
provisioner/scripts/vm-stop.sh "$vmid"
```

Multiple VMs can run concurrently — each gets its own id and its own spec
file. Use `--vm <id>` (or per-shell `TESTANYWARE_VM_ID`) to target a specific one.

### Storage locations

The scripts honour the [XDG Base Directory
spec](https://specification.freedesktop.org/basedir-spec/latest/):

| Content | Path | Purpose |
|---------|------|---------|
| Running-VM spec + metadata | `${XDG_STATE_HOME:-~/.local/state}/testanyware/vms/` | Ephemeral; written by `vm-start.sh`, removed by `vm-stop.sh` |
| QEMU golden images | `${XDG_DATA_HOME:-~/.local/share}/testanyware/golden/` | Persistent; created by `vm-create-golden-windows.sh` |
| QEMU clone working dirs | `${XDG_DATA_HOME:-~/.local/share}/testanyware/clones/<id>/` | Ephemeral; created by `vm-start.sh`, removed by `vm-stop.sh` |
| Windows installer ISO cache | `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/` | Persistent; reused across golden-image builds |

### Per-VM spec file format

`vm-start.sh` writes the spec as JSON. The CLI consumes this; clients
reading it directly can rely on this schema:

```json
{
  "vnc":      { "host": "127.0.0.1", "port": 63530, "password": "..." },
  "agent":    { "host": "192.168.64.2", "port": 8648 },
  "platform": "macos"
}
```

- `vnc` is always present. `vnc.password` is present on tart (macOS/Linux)
  and QEMU (Windows, password `testanyware`).
- `agent` is present when the agent reached health during startup (expected
  for all golden images). Absent if the boot wait timed out.
- `platform` is always present: `macos` | `linux` | `windows`.

Older spec files written by VMs started before the SSH-disable change may
still carry an `ssh` field. The decoder ignores unknown keys, so they load
without modification; new specs do not emit it.

A sibling `<id>.meta.json` is written alongside. It's an internal sidecar
for `vm-stop.sh` (PID, tool, clone dir, viewer window id) — clients should
not read or depend on its shape.

## Golden Image Contents

All images share these properties:

- **User** — `admin`, with autologin to desktop session
- **testanyware-agent** — HTTP service on port 8648, started automatically on boot
- **Solid gray wallpaper** — clean background for screenshot analysis
- **Notifications and widgets disabled** — no visual clutter during tests

### macOS (`testanyware-golden-macos-tahoe`)

- **macOS Tahoe** (Apple Silicon, via Cirrus Labs vanilla image)
- **Agent autostart** — LaunchAgent at `/usr/local/bin/testanyware-agent`
  (label `com.linkuistics.testanyware-agent`)
- **Accessibility** — TCC grant via system TCC database with code signing requirement (SIP disable/enable cycle during image creation)
- **Package manager** — Homebrew (`/opt/homebrew/bin/brew`)
- **Dev tools** — Xcode Command Line Tools (`swift`, `clang`, `git`, `make`)
- **No SSH at runtime** — Remote Login is enabled during golden creation only and turned off (`systemsetup -f -setremotelogin off`) before the final shutdown; clones communicate via the agent on port 8648
- **Session restore disabled** — apps don't reopen old windows
- **SIP enabled** — standard security posture after image creation

### Linux (`testanyware-golden-linux-24.04`)

- **Ubuntu 24.04 Desktop** (ARM64, via Cirrus Labs vanilla image + `ubuntu-desktop-minimal`)
- **Agent autostart** — systemd user service (`testanyware-agent.service`)
- **Accessibility** — AT-SPI2 enabled, `python3-pyatspi` for bindings, `xdotool` for window management fallback
- **Package manager** — apt
- **No SSH at runtime** — `openssh-server` is used during golden creation only and disabled + masked (`systemctl disable ssh && systemctl mask ssh`) immediately before the final shutdown; clones communicate via the agent on port 8648
- **Silent boot** — GRUB hidden, Plymouth splash, no text-mode console output
- **Screen lock and blanking disabled** — no interruptions during tests
- **NetworkManager** — configured via netplan (replaces systemd-networkd from base image)

### Windows (`testanyware-golden-windows-11`)

- **Windows 11 Pro** (ARM64, installed from Microsoft evaluation ISO via QEMU)
- **Agent autostart** — Task Scheduler logon task (`TestAnywareAgent`)
- **Accessibility** — UI Automation via FlaUI (built into Windows)
- **Package manager** — Chocolatey
- **No SSH** — agent binary installed from autounattend media; all communication via agent HTTP
- **First-logon animation disabled** — clones boot straight to desktop without OOBE
- **UEFI + TPM 2.0** — standard Windows 11 secure boot via swtpm
- **VirtIO networking** — virtio-net-pci driver installed during setup

## Requirements

| Requirement | Purpose | Install |
|-------------|---------|---------|
| macOS 14+ | Host OS | — |
| Swift 6.0+ | Build CLI and macOS agent | Included with Xcode 16+, or `xcode-select --install` |
| [tart](https://tart.run) | macOS and Linux VMs | `brew install cirruslabs/cli/tart` |
| [QEMU](https://www.qemu.org/) | Windows VMs | `brew install qemu` |
| [swtpm](https://github.com/stefanberger/swtpm) | TPM 2.0 emulation for Windows 11 | `brew install swtpm` |
| .NET 9+ SDK | Build Windows agent (cross-compiled to `win-arm64`) | `brew install dotnet` |
| Python 3.12+ | Linux agent (ships with Ubuntu desktop) | Not needed on host |
| Python 3.11+ and `uv` | Vision pipeline on the host | `brew install uv` |
| SSH key | macOS/Linux golden image creation | `ssh-keygen -t ed25519` (if `~/.ssh/id_ed25519.pub` doesn't exist) |

### First-run permission

`testanyware vm start --viewer` and `testanyware vm stop` open and close a
VNC viewer via AppleScript. On first run, macOS prompts for **Automation**
permission on behalf of `testanyware`. Grant it under *System Settings →
Privacy & Security → Automation → testanyware → System Events*. Installing
`testanyware` at a stable path (e.g. `/usr/local/bin/testanyware`) keeps
the grant valid across rebuilds.

## Development Conventions

- **Error handling:** Errors propagate via Swift `throws` / `async throws`
  (CLI) or HTTP 400+ responses (agents). Exec output captured as
  `{stdout, stderr, exitCode}`, never thrown.
- **VNC passwords:** Extracted from tart's dynamically-generated VNC URL at
  startup and written into the per-VM spec file by `vm-start.sh`; never
  hardcoded, never hand-assembled.
- **Testing:** Pytest marker-driven (`unit|vision|integration|slow`). Vision
  tests run against golden datasets under `vision/`. Integration tests
  need live VMs from `vm-start.sh`. Always invoke pytest with
  `--import-mode=importlib` to avoid duplicate-module collisions across
  workspace members.
- **Accessibility tree snapshots:** Two modes: `interact` (button/input/text
  only) and `full` (all roles). For layout analysis, prefer `full`. For
  action targeting, use `interact`.
- **Coordinate systems:** All coordinates are screen-absolute. VNC framebuffer
  is (0,0) at top-left; agents return screen coords. Linux GTK4 apps return
  (0,0) for all coordinate types — agent auto-detects and computes offset
  via xdotool + `_GTK_FRAME_EXTENTS`.
- **Text matching:** Word-level OCR (not line-level). Multi-word GT labels
  recovered via adjacent-word fuzzy matching (Levenshtein <= 1,
  case-insensitive).
- **Icon classification:** The learned icon classifier model is **not yet
  trained**. The pipeline currently falls back to a shape-heuristic
  classifier. Expect limited coverage and relatively high false-negative
  rates on icon-only buttons until the model ships.

## Troubleshooting

Known issues, VM quirks, platform-specific workarounds, and CLI edge
cases have moved to [`docs/user/troubleshooting.md`](docs/user/troubleshooting.md).

## Key Directories

```
/cli                         # Host CLI + library (Swift, macOS host)
  /Sources/testanyware/             #   CLI commands & entry point
  /Sources/TestAnywareDriver/       #   VNC/Agent client logic
  /Sources/TestAnywareAgentProtocol/#   Shared HTTP schema (cli-side copy)
  /Tests/                           #   Unit + integration tests

  NOTE: cli/ is flat (no cli/macos/). A cross-platform Rust port is
  pending; until then the Swift package lives directly at cli/.

/agents                      # Platform-specific agents (HTTP servers on port 8648)
  /macos                     #   Swift + Hummingbird; self-contained
    /Sources/testanyware-agent/
    /Sources/TestAnywareAgent/
    /Sources/TestAnywareAgentProtocol/   # agent-side copy; kept in sync by hand with cli-side
  /linux                     #   Python + http.server
    /testanyware_agent/
  /windows                   #   C# + ASP.NET, ships as a .NET 9 win-arm64 binary
    TestAnywareAgent.csproj

/vision                      # Vision pipeline (Python uv workspace)
  /pipeline/                 #   Orchestrator + pipeline driver
  /stages/                   #   Individual analysis stages
    /window-detection/
    /icon-classification/    #   Shape-heuristic fallback (learned model not yet trained)
    /drawing-primitives/
  /common/                   #   Shared detection types & utilities
  /swift/                    #   Swift interop (OCR via Vision framework)
  /docs/                     #   Pipeline-internal notes

/provisioner                 # VM lifecycle + golden-image creation
  /scripts/                  #   vm-start.sh, vm-stop.sh, vm-create-golden-*.sh, etc.
  /autounattend/             #   Windows autounattend.xml + related assets

/docs                        # User, architecture, reference, and component docs
  /user/                     #   quick-start, golden-images, networking, video, troubleshooting
  /architecture/             #   overview, agent-protocol, vision-pipeline, vm-lifecycle
  /reference/                #   cli-commands, env-vars, connection-spec, key-names, error-codes
  /components/               #   maintainer-facing per-component details

/0-docs                      # Design, plan, prompt for this unification
  /designs/TestAnyware-Unification.design.md
  /plans/TestAnyware-Unification.plan.md
  /prompts/TestAnyware-Unification.prompt.md

/LLM_STATE                   # Ravel-Lite plan state (session memory & learnings)
  /core/                     #   general backlog
  /vision-pipeline/          #   OCR/matching insights
  /ocr-accuracy/             #   coordinate fixes, filter ordering
```

## LLM Integration

If you want to drive TestAnyware from another LLM (i.e. use it as a tool
from within an LLM-driven workflow), see
[LLM_INSTRUCTIONS.md](LLM_INSTRUCTIONS.md) for the complete CLI surface,
command reference, vision-pipeline stages, and workflow patterns.
