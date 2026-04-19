# VM Lifecycle

How TestAnyware starts, tracks, and stops VMs across platforms, and
where it keeps the bits on disk.

## Backends

| Platform | Backend | Why |
|----------|---------|-----|
| macOS (Apple Silicon) | [tart](https://tart.run) | Native Apple Virtualization; fastest boot, native HVF |
| Linux (ARM64) | tart | Same — tart handles Linux guests on Apple Silicon |
| Windows 11 (ARM64) | QEMU + swtpm | Windows 11 requires TPM 2.0; tart doesn't provide one |

`testanyware vm start --platform <p>` picks the backend automatically.
`VMLifecycleError.unsupportedBackend` is thrown for platform/host
combinations that have no backend.

## Spec file

Every running VM has exactly one JSON spec file at
`$XDG_STATE_HOME/testanyware/vms/<id>.json` (see full schema at
`docs/reference/connection-spec.md`). It is:

- Written atomically by `testanyware vm start` via `VMSpec.writeAtomic`
  (tmpfile + atomic rename, mode 0600).
- Removed by `testanyware vm stop`.
- Loaded by the CLI whenever `--vm <id>` or `TESTANYWARE_VM_ID` is set.

A sibling `<id>.meta.json` holds PID, backend, clone dir, and viewer
window id — internal only, not for client consumption.

## XDG paths

Resolved by `VMPaths` in `cli/Sources/TestAnywareDriver/VM/VMPaths.swift`.
Defaults follow the XDG Base Directory spec.

| Content | Path | Purpose |
|---------|------|---------|
| Running-VM specs | `${XDG_STATE_HOME:-~/.local/state}/testanyware/vms/<id>.json` and `<id>.meta.json` | Ephemeral; created by `vm start`, removed by `vm stop` |
| QEMU golden images | `${XDG_DATA_HOME:-~/.local/share}/testanyware/golden/` | Persistent; produced by `vm-create-golden-windows.sh` |
| QEMU clone working dirs | `${XDG_DATA_HOME:-~/.local/share}/testanyware/clones/<id>/` | Ephemeral per-clone disk; removed on stop |
| Windows installer ISO cache | `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/` | Persistent; reused across golden-image rebuilds |

tart-managed goldens (macOS, Linux) live under tart's own store
(`~/.tart/vms/`) rather than under `$XDG_DATA_HOME`. `testanyware vm list`
queries both backends and presents a unified view.

## Start flow

1. `testanyware vm start` picks the backend based on `--platform`.
2. **tart path:** `tart clone <golden> <clone-id>` → `tart run` in the
   background → discover VNC URL (`tart vnc <clone-id>`) → parse host,
   port, and dynamically generated password from the URL.
3. **QEMU path:** copy `${XDG_DATA_HOME}/testanyware/golden/<name>/` into
   a fresh `${XDG_DATA_HOME}/testanyware/clones/<id>/` → spawn
   `qemu-system-aarch64` + `swtpm` (for TPM 2.0) in the background →
   discover VNC via the QEMU monitor socket. VNC password is fixed
   (`testanyware`).
4. Wait up to a boot budget (typically 120 s) for VNC to accept
   connections. On timeout, throw `VMLifecycleError.vncTimeout`.
5. Probe `GET /health` on the agent (port 8648) until it returns 200.
   If the agent never responds, the spec is still written but the
   `agent` field is omitted — callers can still drive via VNC.
6. Write `<id>.json` and `<id>.meta.json` atomically; print `<id>` on
   stdout.

## Stop flow

1. Look up `<id>.meta.json` for the backend, clone dir, and viewer
   window id.
2. Send the backend-specific stop (tart: `tart stop <clone-id>` then
   `tart delete <clone-id>`; QEMU: QMP quit then SIGKILL as needed).
3. Close the viewer window if one was opened (AppleScript).
4. Remove the clone dir (QEMU) and both JSON files.

## Golden images

Golden images are built once, then every `vm start` clones from a
golden. Build scripts live at `provisioner/scripts/`:

| Script | Produces |
|--------|----------|
| `vm-create-golden-macos.sh` | `testanyware-golden-macos-tahoe` (tart) |
| `vm-create-golden-linux.sh` | `testanyware-golden-linux-24.04` (tart) |
| `vm-create-golden-windows.sh` | `testanyware-golden-windows-11` (QEMU, under `$XDG_DATA_HOME/testanyware/golden/`) |

Clone semantics: tart uses copy-on-write clones (`tart clone` is fast
and cheap). QEMU uses file-level copy of the backing qcow2 image — one
clone dir per running VM, removed on stop.

The macOS golden creation script temporarily disables SIP (via a
Recovery-boot csrutil cycle) to install a TCC grant for the agent
with code-signing attached; SIP is re-enabled before the image is
finalised. See `provisioner/scripts/vm-create-golden-macos.sh`.

## Multiple concurrent VMs

Any number of VMs can run in parallel — each has its own id and its
own spec file. Every subcommand resolves which VM to target via
`--vm <id>` or `TESTANYWARE_VM_ID`. See
`docs/user/multi-vm-networking.md` for the cookbook.
