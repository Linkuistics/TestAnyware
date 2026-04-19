# Environment Variables

Every variable TestAnyware reads. Unless noted, these can be set in the
parent shell before calling `testanyware` or a script under
`provisioner/scripts/`.

## `TESTANYWARE_*` — connection & runtime

| Name | Purpose | Default | Consumer |
|------|---------|---------|----------|
| `TESTANYWARE_VM_ID` | VM instance id; resolves to `$XDG_STATE_HOME/testanyware/vms/<id>.json` | unset | CLI (`testanyware` all subcommands); `provisioner/scripts/vm-stop.sh` fallback |
| `TESTANYWARE_VNC` | VNC endpoint `host[:port]` (ad-hoc; no spec file needed). Lower priority than `TESTANYWARE_VM_ID` | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_VNC_PASSWORD` | VNC password used with `TESTANYWARE_VNC` | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_AGENT` | Agent HTTP endpoint `host[:port]`. Overrides the spec file's agent | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_PLATFORM` | Target platform: `macos`, `linux`, `windows` | unset | `ConnectionSpec.fromEnvironment()` |
| `TESTANYWARE_OCR_FALLBACK` | Set to `1` to force the EasyOCR child-bridge path on macOS (otherwise macOS uses in-process Apple Vision) | `0` | `TestAnywareServer` OCR selection |
| `TESTANYWARE_OCR_PYTHON` | Override the Python interpreter used for the EasyOCR daemon | `python3` on PATH | `TestAnywareServer` OCR bridge |
| `TESTANYWARE_VNC_DEBUG` | Set to `1` to emit verbose RFB/VNC protocol logging to stderr | `0` | `VNCCapture` |
| `TESTANYWARE_VNC_ARD_REMAP` | Set to `1` to force Apple Remote Desktop keycode remapping (usually auto-detected) | `0` | `VNCInput` |
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
own managed location (`~/.tart/vms/`); only Windows (QEMU) goldens live
under `$XDG_DATA_HOME/testanyware/golden/`.

## Connection resolution order

`ConnectionOptions.resolve()` in `cli/Sources/testanyware/TestAnywareCLI.swift`
tries, in order:

1. `--connect <path>` — explicit spec file
2. `--vm <id>` — per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json`
3. `--vnc` / `--agent` / `--platform` — explicit flags
4. `TESTANYWARE_VM_ID` — resolves to the per-VM spec like `--vm`
5. `TESTANYWARE_VNC` / `TESTANYWARE_VNC_PASSWORD` / `TESTANYWARE_AGENT` /
   `TESTANYWARE_PLATFORM` — direct env vars
6. Error
