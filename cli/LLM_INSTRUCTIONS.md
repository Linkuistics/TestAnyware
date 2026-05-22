# TestAnyware — LLM Instructions

`testanyware` is a command-line tool that drives a virtual machine for UI
testing. It reaches each VM over two channels: **VNC** (screenshots,
keyboard, mouse, video) and an in-VM **HTTP agent** on port 8648
(accessibility tree, semantic UI actions, command exec, file transfer).

It is *not* a general remote shell, *not* a host-side screen recorder, and
*not* an OCR library — it is glue between an LLM agent and a running guest
OS. Every instruction below is runnable with only the `testanyware` binary
on your `PATH`; no source checkout is required.

## Command surface

Most commands are top-level verbs (`screenshot`, `exec`, `record`, …).
Three groups collect related commands: `agent` (in-VM accessibility),
`vm` (VM lifecycle), and `input` (keyboard and mouse).

Run `testanyware --help` for the full command list, and
`testanyware <command> --help` for any command's options, arguments, and
examples. This guide is the map; `--help` is the authoritative reference.

## Connecting to a VM

Every VM-touching command needs a connection target, resolved in this
order (first match wins):

1. `--connect <path>` — explicit JSON spec file.
2. `--vm <id>` — per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json`.
3. `--vnc <host:port>` (with `--agent`, `--platform`) — direct flags.
4. `TESTANYWARE_VM_ID` env — resolves like `--vm`.
5. `TESTANYWARE_VNC` / `TESTANYWARE_AGENT` / `TESTANYWARE_PLATFORM` env.
6. Otherwise: error.

Typical pattern: start a VM, export its id once, then run later commands
with no connection flags.

## Quick start

```bash
vmid=$(testanyware vm start --platform macos)   # also: linux | windows
export TESTANYWARE_VM_ID="$vmid"
testanyware screenshot -o screen.png
testanyware vm stop "$vmid"
```

`vm start` prints the new VM id on stdout.

### VM lifecycle

- `testanyware vm start [--platform macos|linux|windows]` — start a VM; prints its id. See `vm start --help` for id, display, and viewer options.
- `testanyware vm stop <id>` — stop a VM and remove its spec file.
- `testanyware vm list` — list golden images and running clones.
- `testanyware vm delete <name>` — delete a golden image.

`vm start` boots a clone of a pre-built **golden image**. Run `testanyware
vm list` to see which images are available. If none exist, create one with
the golden-image scripts bundled alongside the CLI:

```bash
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-macos.sh"
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-linux.sh"
bash "$(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-windows.sh" --iso ~/Downloads/Win11_ARM64.iso
```

## Agent commands (semantic / exec channel)

```bash
testanyware agent health                       # agent reachable + accessibility status
testanyware agent windows                      # list visible windows
testanyware agent snapshot [--json]            # accessibility element tree
testanyware agent inspect [query] [--json]     # one element: properties + bounds
testanyware agent wait                         # block until accessibility is ready
testanyware agent press [query]                # press / click an element
testanyware agent set-value [query] --value T  # set text / slider value
testanyware agent focus [query]                # focus an element
testanyware agent show-menu [query]            # open a context menu
testanyware agent window-focus|window-resize|window-move|window-close|window-minimize ...
```

Element query options: `--role ROLE`, `--label TEXT`, `--window FILTER`.
`agent snapshot` and `agent inspect` accept `--json` for machine-readable
output; the other agent commands print text.

The in-VM agent needs OS accessibility permission; golden images grant it
at build time. `agent health` reports whether it is granted.

### exec & file transfer

```bash
testanyware exec "command"                     # run a shell command in the guest
testanyware exec "command" --detach            # launch detached, return immediately
testanyware upload   <local> <remote>
testanyware download <remote> <local>
```

`exec` exits with the guest command's own exit code. Prefer `exec` over
SSH — it is the supported in-guest exec channel.

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
testanyware screen-size                        # "WIDTHxHEIGHT"
testanyware screenshot -o file.png             # full screen
testanyware screenshot -o file.png --region x,y,width,height
testanyware screenshot -o file.png --window "Window Name"
testanyware record -o out.mp4 --fps 30 --duration 10
testanyware find-text "search text"            # OCR; emits JSON with coordinates
testanyware find-text "Loading" --timeout 30   # poll until the text appears
```

`find-text` emits JSON like
`[{"text":"Terminal","x":248,"y":91,"width":55,"height":12,"confidence":0.95}]`.
Click the element's centre at `x + width/2`, `y + height/2`.

## Connection spec JSON

`testanyware vm start` writes a per-VM spec file that `--vm <id>` (and
`--connect <path>`) consume:

```json
{"vnc":      {"host": "localhost", "port": 5900, "password": "secret"},
 "agent":    {"host": "192.168.64.100", "port": 8648},
 "platform": "macos"}
```

Spec files live at
`${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/<id>.json` (mode
`600` — treat them as secrets).

## Environment variables

| Variable | Purpose |
|----------|---------|
| `TESTANYWARE_VM_ID` | Resolves to the per-VM spec file |
| `TESTANYWARE_AGENT` | Agent HTTP endpoint `host:port` (overrides the spec) |
| `TESTANYWARE_VNC` | VNC endpoint `host:port` (ad-hoc; no spec file) |
| `TESTANYWARE_VNC_PASSWORD` | VNC password used with `TESTANYWARE_VNC` |
| `TESTANYWARE_PLATFORM` | Target platform (`macos`, `linux`, `windows`) |

## Workflow patterns

All examples assume `TESTANYWARE_VM_ID` is set.

**Discover-then-act (preferred)** — the agent is the primary channel; act
on elements semantically, not by pixel:

```bash
testanyware agent snapshot                              # 1. see the UI
testanyware agent press --role button --label "Save"    # 2. act by role + label
testanyware agent snapshot                              # 3. verify the result
```

**Visual verification** — for colours, layout, and custom-drawn UI that
accessibility cannot see:

```bash
testanyware screenshot -o screen.png
testanyware find-text "Expected text"
```

**Install software in the guest:**

```bash
testanyware exec "brew install jq"             # macOS
testanyware exec "sudo apt-get install -y jq"  # Linux
testanyware exec "choco install jq -y"         # Windows
```

## Common mistakes

- **Don't click pixel coordinates when a role + label exists.** `agent press --role button --label "Save"` survives layout changes; pixel clicks do not.
- **Don't reuse a screenshot's coordinates after a window moves.** Re-snapshot, or use `--window` filters that resolve at action time.
- **Don't mix `--vm` with `--vnc`/`--agent`.** The resolution chain takes the first match and ignores the rest.
- **Sleep after input.** VMs need time to settle: `sleep 1` after keyboard/mouse, `sleep 5` after launching an app.

## Exit codes

`0` on success, non-zero on failure. `exec` propagates the guest command's
own exit code — so a non-zero exit from `exec` may come from your command,
not from `testanyware` itself.

## Where to read more

- `testanyware --help` — the full command list.
- `testanyware <command> --help` — a command's options, arguments, and examples.
- `testanyware doctor` — diagnose the local install.
