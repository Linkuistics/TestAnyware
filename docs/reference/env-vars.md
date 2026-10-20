# Environment Variables

Every variable TestAnyware reads. Unless noted, these can be set in the
parent shell before calling `testanyware` or a script under
`provisioner/scripts/`.

## `TESTANYWARE_*` — connection & runtime

| Name | Purpose | Default | Consumer |
|------|---------|---------|----------|
| `TESTANYWARE_VM_ID` | VM instance id. It resolves to `$XDG_STATE_HOME/testanyware/vms/<id>.json` | unset | CLI (`testanyware` all subcommands). `provisioner/scripts/vm-stop.sh` uses it as a fallback |
| `TESTANYWARE_VNC` | VNC endpoint `host[:port]` (ad-hoc, no spec file needed). Lower priority than `TESTANYWARE_VM_ID` | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_VNC_PASSWORD` | VNC password used with `TESTANYWARE_VNC` | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_AGENT` | Agent HTTP endpoint `host[:port]`. Lower priority than `TESTANYWARE_VM_ID`: a spec file always beats it | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_PLATFORM` | Target platform: `macos`, `linux`, `windows` | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_CONFIG` | Another `config.rhai` file for the Host CLI, or `none` to run without one. See [config.rhai](#configrhai). The app does not read this variable | `~/.config/testanyware/config.rhai` | CLI (`testanyware`, every host) |
| `TESTANYWARE_APP_SOCKET` | **Internal / test-only.** Socket of the TestAnyware app. On a macOS host it replaces the default socket, `~/Library/Application Support/TestAnyware/testanyware.sock`. On a Linux or Windows host it turns on routing through the app. A macOS host needs no variable | unset | CLI (`testanyware`) |
| `TESTANYWARE_OCR_FALLBACK` | Set to `1` to force the EasyOCR child-bridge path on macOS (otherwise macOS uses in-process Apple Vision) | `0` | `TestAnywareServer` OCR selection |
| `TESTANYWARE_OCR_PYTHON` | Override the Python interpreter used for the EasyOCR daemon | `python3` on PATH | `TestAnywareServer` OCR bridge |
| `TESTANYWARE_VNC_DEBUG` | Set to `1` to emit verbose RFB/VNC protocol logging to stderr | `0` | `VNCCapture` |
| `TESTANYWARE_VNC_ARD_REMAP` | Set to `1` to force Apple Remote Desktop keycode remapping (usually auto-detected) | `0` | `VNCInput` |
| `TESTANYWARE_RFB_ENCODING` | **Internal / test-only.** Force the RFB client to advertise one primary encoding: `zrle`, `tight`, or `raw`. CopyRect and the desktop-size and last-rect pseudo-encodings stay. The live-VM gate uses it to test each decoder alone against a real server. An unrecognised value fails with `USAGE_ERROR` (exit 2). | unset (full `zrle`>`tight`>`copyrect`>`raw` preference) | `testanyware-rfb` (`RfbConnection::handshake`) |
| `TESTANYWARE_SKIP_INTEGRATION` | Set to `1` to skip all integration tests in `cli/Tests/IntegrationTests/` | `0` | `VNCIntegrationTests`, `VMLifecycleTests` |
| `TESTANYWARE_SKIP_VIEWER_TEST` | Set to `1` to opt out of the viewer open/close integration test | `0` | `VMLifecycleTests` |
| `TESTANYWARE_SERVER_URL` | Base URL for the collect-training-data helper against a running local server | `http://localhost:9100` | `vision/stages/icon-classification/training/collect-training-data.sh` |

## XDG Base Directory variables

TestAnyware honours the XDG Base Directory spec for all persistent and
ephemeral state it writes. Defaults are derived from `$HOME`.

| Name | Default | Used for |
|------|---------|----------|
| `XDG_STATE_HOME` | `$HOME/.local/state` | `testanyware/vms/<id>.json` (per-VM running spec) and sibling `<id>.meta.json` |
| `XDG_DATA_HOME` | `$HOME/.local/share` | `testanyware/golden/` (QEMU golden images), `testanyware/clones/<id>/` (per-clone QEMU working dirs), `testanyware/cache/` (Windows ISO cache) |

macOS/Linux (tart) golden images live outside this tree, under tart's
own managed location (`~/.tart/vms/`). Only Windows (QEMU) goldens live
under `$XDG_DATA_HOME/testanyware/golden/`.

## Connection resolution order

The host CLI's connection resolution (`cli-rs/crates/testanyware-cli/src/resolve.rs`)
tries, in order:

1. `--connect <path>` — explicit spec file
2. `--vm <id>`, or `TESTANYWARE_VM_ID` — per-VM spec at
   `$XDG_STATE_HOME/testanyware/vms/<id>.json`
3. `--vnc` / `--agent`, or `TESTANYWARE_VNC` / `TESTANYWARE_AGENT` — explicit
   endpoints. `TESTANYWARE_VNC_PASSWORD` gives the password of `--vnc`
4. Error

Each variable is the value of its flag, so a variable and its flag take the same
place. A VM id, whether from `--vm` or from `TESTANYWARE_VM_ID`, comes before
`--agent` and `--vnc`. `--platform` and `TESTANYWARE_PLATFORM` set the platform.
They do not choose a target. On a macOS host, the TestAnyware app opens the
connection that the chain names. The chain does not change.

## config.rhai

One optional file, `~/.config/testanyware/config.rhai`, configures the Host CLI
and the TestAnyware app. Its value is a map. Every setting is a string that
holds an absolute path:

| Key | Meaning |
|-----|---------|
| `tools` | A map from `tart`, `qemu-system-aarch64` or `swtpm` to the location of that tool. The Host CLI finds a tool that the file names at that location, or not at all |
| `state_dir` | The state directory itself, the one that holds `vms/`. It replaces `XDG_STATE_HOME` plus `testanyware` |
| `data_dir` | The data directory itself, the one that holds `golden/`. It replaces `XDG_DATA_HOME` plus `testanyware` |

An unknown key is an error, and so is an unknown tool. A flag beats an
environment variable, which beats the file, which beats the built-in default.
The Host CLI and the app must resolve the same two directories. If they differ,
a command fails with `APP_INCOMPATIBLE` and cause `directories`. A file that
fails gives `CONFIG_INVALID`, except in `testanyware doctor`, which reports it.
The app reads the file when it starts, so quit TestAnyware and open it again
after an edit.
