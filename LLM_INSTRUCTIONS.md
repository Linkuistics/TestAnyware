# TestAnyware — LLM Instructions

`testanyware` is a command-line tool that drives a virtual machine for UI
testing. It reaches each VM over two channels: **VNC** (screenshots,
keyboard, mouse, video) and an in-VM **HTTP agent** on port 8648
(accessibility tree, semantic UI actions, command exec, file transfer).

It is *not* a general remote shell, *not* a host-side screen recorder, and
*not* an OCR library — it is glue between an LLM agent and a running guest
OS. Every instruction below is runnable with only the `testanyware` binary
on your `PATH`; no source checkout is required.

## Mental model

The command tree is **noun-first**, with verb-first aliases kept for
convenience:

- Noun groups: `vm`, `agent`, `input`, `screen`, `file`.
- Utilities: `doctor`, `capabilities`, `schema`, `llm-instructions`.
- Verb-first aliases: `screenshot`→`screen capture`, `record`→`screen
  record`, `screen-size`→`screen size`, `find-text`→`screen find-text`,
  `upload`/`download`/`exec`→`file upload`/`download`/`exec`.

**Connection resolution** — every VM-touching command needs a target,
resolved in this order (first match wins):

1. `--connect <path>` — explicit JSON spec file.
2. `--vm <id>` — per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json`.
3. `--vnc` / `--agent` / `--platform` — direct flags.
4. `TESTANYWARE_VM_ID` env — resolves like `--vm`.
5. `TESTANYWARE_VNC` / `TESTANYWARE_AGENT` / `TESTANYWARE_PLATFORM` env.
6. Otherwise: error.

The typical pattern is to start a VM, export its id once, then run every
later command with no connection flags.

## Quick start

```bash
vmid=$(testanyware vm start --platform macos)   # also: linux | windows
export TESTANYWARE_VM_ID="$vmid"
testanyware screen capture -o screen.png
testanyware vm stop "$vmid"
```

`vm start` prints the new VM id on stdout. When automation spans multiple
processes, persist the id to a file so later steps can find and stop it:

```bash
vmid=$(testanyware vm start)
printf '%s\n' "$vmid" > .testanyware-vmid
testanyware screen capture --vm "$(cat .testanyware-vmid)" -o out.png
testanyware vm stop "$(cat .testanyware-vmid)"
rm .testanyware-vmid
```

### VM lifecycle

- `testanyware vm start [--platform macos|linux|windows] [--id <id>] [--display WxH] [--viewer]` — start a VM; prints its id.
- `testanyware vm stop <id>` — stop a VM and remove its spec file.
- `testanyware vm list` — list golden images and running clones.
- `testanyware vm delete <name> [--force]` — delete a golden image.

`vm start` boots a **clone of a pre-built golden image** (clean desktop,
agent pre-installed, accessibility enabled, agent on port 8648). Run
`testanyware vm list` to see which golden images are available. If none
exist, create one with the golden-image scripts bundled alongside the CLI
(installed under `share/testanyware/scripts/`):

```bash
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-macos.sh"
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-linux.sh"
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-windows.sh" --iso ~/Downloads/Win11_ARM64.iso
```

`vm start --viewer` opens a VNC viewer window; on macOS the first use
prompts for Automation permission.

## Agent commands (semantic / exec channel)

```bash
testanyware agent health                       # agent reachable + accessibility status
testanyware agent windows                      # list visible windows
testanyware agent snapshot [options]           # accessibility tree
testanyware agent inspect [query]              # one element: properties + bounds
testanyware agent wait [query] [--timeout S]   # block until an element appears
testanyware agent press [query]                # press / click an element
testanyware agent set-value [query] --value T  # set text / slider value
testanyware agent focus [query]                # focus an element
testanyware agent show-menu [query]            # open a context menu
testanyware agent window-focus|window-resize|window-move|window-close|window-minimize --window FILTER ...
```

- Snapshot options: `--mode interact|layout|full`, `--window FILTER`, `--role ROLE`, `--label TEXT`, `--depth N`.
- Element query: `--role ROLE`, `--label TEXT`, `--window FILTER`, `--id ID`, `--index N`.

The in-VM agent needs OS-level accessibility permission; golden images
grant it at build time. `agent health` reports whether it is granted —
a missing grant surfaces as exit code `4` (`AUTH_REQUIRED`).

### exec & file transfer

```bash
testanyware file exec "command"                # run a shell command in the guest
testanyware file upload   localpath remotepath
testanyware file download remotepath localpath
```

Prefer `file exec` over SSH — it is the supported in-guest exec channel.

## Input commands (VNC channel)

```bash
testanyware input key KEYNAME [--modifiers mod1,mod2]
testanyware input key-down KEYNAME             # hold
testanyware input key-up   KEYNAME             # release
testanyware input type "text to type"
testanyware input click X Y [--button left|right|middle] [--count 2]
testanyware input mouse-down|mouse-up X Y [--button left|right|middle]
testanyware input move X Y
testanyware input scroll X Y --dy -3           # negative dy = scroll up
testanyware input drag fromX fromY toX toY
```

- Keys: `a`-`z` `0`-`9` `return` `tab` `escape` `space` `delete` `backspace` `forwarddelete` `up` `down` `left` `right` `home` `end` `pageup` `pagedown` `f1`-`f19`.
- Modifiers: `cmd` `alt` `shift` `ctrl`, mapped per `--platform` (`macos` maps `cmd` to Command; `windows` maps `cmd` to Ctrl).

## Screen commands (VNC channel)

```bash
testanyware screen size                        # "1920x1080"
testanyware screen capture -o file.png         # full screen
testanyware screen capture --region x,y,w,h -o file.png
testanyware screen record  -o out.mp4 --fps 30 --duration 10
testanyware screen find-text "search text"     # OCR; returns JSON with coords
testanyware screen find-text "Loading" --timeout 30   # poll until found
testanyware screen find-text                   # all text on screen
```

`find-text` returns e.g.
`[{"text":"Terminal","x":248,"y":91,"width":55,"height":12,"confidence":0.95}]`.
Click the element's center at `x + width/2`, `y + height/2`.

## Connection spec JSON

`testanyware vm start` writes a per-VM spec file that `--vm <id>` (and
`--connect <path>`) consume:

```json
{"vnc":      {"host": "localhost", "port": 5900, "password": "secret"},
 "agent":    {"host": "192.168.64.100", "port": 8648},
 "platform": "macos"}
```

- `vnc` is always present; `vnc.password` is present on the macOS, Linux, and Windows goldens.
- `agent` is present once the agent reached health during startup.
- `platform` is one of `macos` | `linux` | `windows`.

Spec files live at
`${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/<id>.json`, mode
`600` — treat them as secrets.

## Environment variables

| Variable | Example | Purpose |
|----------|---------|---------|
| `TESTANYWARE_VM_ID` | `testanyware-a3f7b2c1` | Resolves to the per-VM spec file |
| `TESTANYWARE_AGENT` | `192.168.64.100:8648` | Agent HTTP endpoint (overrides the spec) |
| `TESTANYWARE_VNC` | `127.0.0.1:59948` | VNC endpoint (ad-hoc; no spec file needed) |
| `TESTANYWARE_VNC_PASSWORD` | `syrup-rotate-nasty` | VNC password used with `TESTANYWARE_VNC` |
| `TESTANYWARE_PLATFORM` | `macos` | Target platform (`macos`, `linux`, `windows`) |

## Workflow patterns

All examples assume `TESTANYWARE_VM_ID` is set.

**Discover-then-act (preferred)** — the agent is the primary channel; act
on elements semantically, not by pixel:

```bash
testanyware agent snapshot --mode interact            # 1. see the UI semantically
testanyware agent press --role button --label "Save"  # 2. act by role + label
testanyware agent snapshot --mode interact            # 3. verify the result
```

**Visual verification** — for colors, layout, and custom-drawn UI that
accessibility cannot see:

```bash
testanyware screen capture -o screen.png
testanyware screen find-text "Expected text"
```

**Install software in the guest:**

```bash
testanyware file exec "brew install jq"             # macOS
testanyware file exec "sudo apt-get install -y jq"  # Linux
testanyware file exec "choco install jq -y"         # Windows
```

## JSON output, exit codes, idempotency

- Every data-producing command supports `--json`. JSON envelopes carry `schema_version` and `ok`; failures add a stable `code` and a `message`.
- Exit codes: `0` success, `1` generic, `2` usage, `3` not-found, `4` auth, `5` conflict, `7` timeout. `file exec` propagates the guest process's own exit code.
- Mutating commands accept `--dry-run`: the JSON envelope sets `dry_run: true` and no side effect runs.

## Common mistakes

- **Don't parse text output.** Use `--json` and parse the envelope — text output is for humans and is not a stable interface.
- **Don't click pixel coordinates when a role + label exists.** `agent press --role button --label "Save"` survives layout changes; pixel clicks do not.
- **Don't reuse a screenshot's coordinates after a window moves.** Coordinates are absolute; re-snapshot, or use `--window <name>` filters that resolve at click time.
- **Don't poll `agent windows` in a tight loop.** Use `agent wait`, which blocks until the element is ready.
- **Don't mix `--vm` with `--vnc`/`--agent`.** The resolution chain takes the first match and silently ignores the rest.
- **Sleep after input.** VMs need time to settle: `sleep 1` after keyboard/mouse, `sleep 5` after launching an app.

## Where to read more

- `testanyware capabilities` — machine-readable surface: every subcommand, alias, feature flag, and error code.
- `testanyware schema <command>` — JSON Schema for a command's `--json` output, e.g. `testanyware schema agent snapshot`.
- `testanyware <command> --help` — full per-command help: usage, output formats, exit codes, examples.
- `testanyware doctor` — diagnose the local install when CLI behaviour looks mismatched with the installed version.
