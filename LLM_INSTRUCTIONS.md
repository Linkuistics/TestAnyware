# TestAnyware — LLM Instructions

`testanyware` is a command-line tool that drives a virtual machine for UI
testing. It reaches each VM over two channels. The first is **VNC**
(screenshots, keyboard, mouse, video). The second is an in-VM **HTTP agent**
on port 8648 (accessibility tree, semantic UI actions, command exec, file
transfer).

It is *not* a general remote shell, *not* a host-side screen recorder, and
*not* an OCR library. It is glue between an LLM agent and a running guest
OS. Every instruction below is runnable with only the `testanyware` binary
on your `PATH`. You do not need a source checkout.

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
2. `--vm <id>`, or the `TESTANYWARE_VM_ID` env var — per-VM spec.
3. `--vnc` / `--agent`, or the `TESTANYWARE_VNC` / `TESTANYWARE_AGENT` env vars — direct endpoints.
4. Otherwise: error.

An env var takes the place of its flag. So `TESTANYWARE_VM_ID` beats `--agent`.
`--platform` and `TESTANYWARE_PLATFORM` set the platform. They do not choose a target.

The typical pattern is to start a VM, export its id once, then run every
later command with no connection flags.

## Quick start

```bash
vmid=$(testanyware vm start --platform macos)   # also: linux | windows
export TESTANYWARE_VM_ID="$vmid"
testanyware screen capture -o screen.png
testanyware vm stop "$vmid"
```

`vm start` prints the new VM id on stdout. For a QEMU-backed guest (Windows)
it also says on stderr whether it attached or omitted the guest's TPM.
`--json` carries the same answer as a `tpm` object. When automation spans multiple
processes, persist the id to a file so later steps can find and stop it:

```bash
vmid=$(testanyware vm start)
printf '%s\n' "$vmid" > .testanyware-vmid
testanyware screen capture --vm "$(cat .testanyware-vmid)" -o out.png
testanyware vm stop "$(cat .testanyware-vmid)"
rm .testanyware-vmid
```

### VM lifecycle

- `testanyware vm start [--platform macos|linux|windows] [--id <id>] [--display WxH] [--viewer]` — start a VM. It prints its id.
- `testanyware vm stop <id>` — stop a VM and delete its spec file.
- `testanyware vm list` — list golden images and running clones.
- `testanyware vm delete <name> [--force]` — delete a golden image.

`vm start` boots a **clone of a pre-built golden image** (clean desktop,
agent pre-installed, accessibility enabled, agent on port 8648). Run
`testanyware vm list` to see which golden images are available. If none
exist, create one. All three goldens (macOS, Linux, Windows) are
in-process `testanyware vm create-golden --platform <os>` commands. No
golden shell scripts ship any more. The Windows golden builds Windows 11
ARM64 (`testanyware-golden-windows-11`) via QEMU+swtpm from a Microsoft
evaluation ISO, so its first run needs `--iso <path>`:

```bash
testanyware vm create-golden --platform macos
testanyware vm create-golden --platform linux
testanyware vm create-golden --platform windows --iso ~/Downloads/Win11_ARM64.iso
```

`vm start --viewer` opens a VNC viewer window. On macOS the first use
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

The in-VM agent needs OS-level accessibility permission. Golden images
grant it at build time. `agent health` reports whether the agent has this
permission. A missing grant surfaces as exit code `4` (`AUTH_REQUIRED`).

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
- Modifiers: `cmd` `alt` `shift` `ctrl`. The tool maps them per `--platform`. The `macos` platform maps `cmd` to Command. The `windows` platform maps `cmd` to Ctrl.

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
Click the element's centre at `x + width/2`, `y + height/2`.

## Connection spec JSON

`testanyware vm start` writes a per-VM spec file that `--vm <id>` (and
`--connect <path>`) consume. Use the file through those options only. Do not
read it yourself, because it is private to TestAnyware. Use `vm list` to find an id.

```json
{"vnc":      {"host": "localhost", "port": 5900, "password": "secret"},
 "agent":    {"host": "192.168.64.100", "port": 8648},
 "platform": "macos"}
```

- `vnc` is always present. `vnc.password` is present on the macOS, Linux, and Windows goldens.
- `agent` is present once the agent reached health during startup.
- `platform` is one of `macos` | `linux` | `windows`.

Spec files live at
`${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms/<id>.json`, mode
`600` — treat them as secrets.

## The TestAnyware app (macOS host)

On a macOS host, every command that reaches a guest goes through the
**TestAnyware app**, even with an explicit `--agent`, `--vnc` or `--connect`.
The app is a menu bar app that holds the Local Network access. So an agent that
macOS refuses, for example under herdr, can still drive a VM. Only
`vm create-golden` connects directly. Linux and Windows hosts have no app.

**A person sets the app up once.** An agent cannot.

1. `brew install --cask linkuistics/taps/testanyware-app`.
2. Open TestAnyware from `/Applications`. Answer the "downloaded from the Internet" dialog with Open.
3. Turn on "Start at Login" in the app's menu.
4. Allow the Local Network alert when it appears.

An upgrade of the app can ask for the Local Network access again.

**A client never starts the app.** When the app is not running, the command
fails and names the remedy. Tell the person to open TestAnyware. Do not retry in
a loop. Read `remediation` in the `--json` envelope.

| Code (exit) and `details.cause` | Remedy |
|---|---|
| `APP_NOT_INSTALLED` (1): no `TestAnyware.app` in `/Applications` or `~/Applications` | A person does the setup above |
| `APP_UNREACHABLE` (1): `not_running` or `quitting` | Open TestAnyware from `/Applications` |
| `APP_UNREACHABLE` (1): `unsafe_socket_directory` | Make the app's socket directory yours with mode 0700, then open TestAnyware |
| `APP_INCOMPATIBLE` (1): `protocol` | Do what `details.remedy` says: `quit_and_open_app`, `upgrade_cask` or `upgrade_host_cli` |
| `APP_INCOMPATIBLE` (1): `directories` | Make `XDG_STATE_HOME` and `XDG_DATA_HOME`, or `state_dir` and `data_dir` in `config.rhai`, the same for the CLI and the app. Then quit and open TestAnyware |
| `CONFIG_INVALID` (2) | Fix `~/.config/testanyware/config.rhai`, or delete it. `TESTANYWARE_CONFIG=none` runs one command without it |
| `CONNECTION_REFUSED` or `VM_BOOT_TIMEOUT` (7): `local_network_denied` | A person allows TestAnyware in System Settings, Privacy & Security, Local Network |

`config.rhai` is optional. Its keys are `tools` (`tart`, `qemu-system-aarch64`,
`swtpm`), `state_dir` and `data_dir`, each an absolute path.

**Golden creation** connects to the guest itself. Run `vm create-golden` from
Terminal, which has Local Network access. Under a harness that macOS refuses, it
fails with the Local Network error.

`testanyware doctor` reports the app: installed, running, started at login and
allowed on the Local Network. Each failing item has a remedy. Its `config` check
reports `config.rhai`.

## Environment variables

| Variable | Example | Purpose |
|----------|---------|---------|
| `TESTANYWARE_VM_ID` | `testanyware-a3f7b2c1` | Resolves to the per-VM spec file. Same place as `--vm` |
| `TESTANYWARE_AGENT` | `192.168.64.100:8648` | Agent HTTP endpoint. Used when no spec file resolves |
| `TESTANYWARE_VNC` | `127.0.0.1:59948` | VNC endpoint (ad-hoc, no spec file needed) |
| `TESTANYWARE_VNC_PASSWORD` | `syrup-rotate-nasty` | VNC password used with `TESTANYWARE_VNC` |
| `TESTANYWARE_PLATFORM` | `macos` | Target platform (`macos`, `linux`, `windows`) |
| `TESTANYWARE_CONFIG` | `none` | Another `config.rhai` file, or `none` to run without one |

## Workflow patterns

Set `TESTANYWARE_VM_ID` before you run these examples.

**Discover-then-act (preferred)** — the agent is the primary channel. Act
on elements semantically, not by pixel:

```bash
testanyware agent snapshot --mode interact            # 1. see the UI semantically
testanyware agent press --role button --label "Save"  # 2. act by role + label
testanyware agent snapshot --mode interact            # 3. verify the result
```

**Visual verification** — for colours, layout, and custom-drawn UI that
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

- Every data-producing command supports `--json`. JSON envelopes carry `schema_version` and `ok`. Failures add a stable `code` and a `message`.
- Exit codes: `0` success, `1` generic, `2` usage, `3` not-found, `4` auth, `5` conflict, `7` timeout. `file exec` propagates the guest process's own exit code.
- Mutating commands accept `--dry-run`: the JSON envelope sets `dry_run: true` and no side effect runs.

## Common mistakes

- **Don't parse text output.** Use `--json` and parse the envelope — text output is for humans and is not a stable interface.
- **Don't click pixel coordinates when a role + label exists.** `agent press --role button --label "Save"` survives layout changes. Pixel clicks do not.
- **Don't reuse a screenshot's coordinates after a window moves.** Coordinates are absolute. Re-snapshot, or use `--window <name>` filters that resolve at click time.
- **Don't poll `agent windows` in a tight loop.** Use `agent wait`, which blocks until the element is ready.
- **Don't mix `--vm` with `--vnc`/`--agent`.** The resolution chain takes the first match and silently ignores the rest.
- **Sleep after input.** VMs need time to settle: `sleep 1` after keyboard/mouse, `sleep 5` after starting an app.
- **On a macOS 27 host, a tart guest (macOS or Linux) dies if a `screen` or `input` command runs 60 seconds after the last one.** Tart's VNC server aborts on a reconnect after an idle gap. `vm start` prevents this. It starts a VNC keeper (`testanyware _vnc-keeper`) that holds one silent connection for the guest's life, so do not kill it. A tart guest that you started with `tart run` has no keeper. For that guest, keep one VNC connection open yourself (`tail -f /dev/null | nc <vnc-host> <vnc-port>`), or keep commands less than 60 seconds apart. See `docs/user/troubleshooting.md`.

## Where to read more

- `testanyware capabilities` — machine-readable surface: every subcommand, alias, feature flag, and error code.
- `testanyware schema <command>` — JSON Schema for a command's `--json` output, e.g. `testanyware schema agent snapshot`.
- `testanyware <command> --help` — full per-command help: usage, output formats, exit codes, examples.
- `testanyware doctor` — diagnose the local install when CLI behaviour looks mismatched with the installed version. On a macOS host it also reports the TestAnyware app.
