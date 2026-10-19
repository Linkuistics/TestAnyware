# VM Lifecycle

How TestAnyware starts, tracks, and stops VMs across platforms, and
where it keeps the bits on disk.

## Backends

| Platform | Backend | Why |
|----------|---------|-----|
| macOS (Apple Silicon) | [tart](https://tart.run) | Native Apple Virtualization, with the fastest boot and native HVF |
| Linux (ARM64) | tart | Same — tart handles Linux guests on Apple Silicon |
| Windows 11 (ARM64) | QEMU + swtpm | Windows 11 requires TPM 2.0. tart does not provide one |

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
- Loaded by the CLI whenever the user sets `--vm <id>` or `TESTANYWARE_VM_ID`.

A sibling `<id>.meta.json` holds PID, backend, clone dir, and viewer
window id — internal only, not for client consumption.

## XDG paths

Resolved by the path helpers in `cli-rs/crates/testanyware-vm/src/paths.rs`.
Defaults follow the XDG Base Directory spec.

| Content | Path | Purpose |
|---------|------|---------|
| Running-VM specs | `${XDG_STATE_HOME:-~/.local/state}/testanyware/vms/<id>.json` and `<id>.meta.json` | Ephemeral. `vm start` creates them and `vm stop` removes them |
| QEMU golden images | `${XDG_DATA_HOME:-~/.local/share}/testanyware/golden/` | Persistent. `testanyware vm create-golden --platform windows` produces them |
| QEMU clone working dirs | `${XDG_DATA_HOME:-~/.local/share}/testanyware/clones/<id>/` | Ephemeral per-clone disk. `vm stop` removes it |
| Windows installer ISO cache | `${XDG_DATA_HOME:-~/.local/share}/testanyware/cache/` | Persistent. Golden-image rebuilds reuse it |

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
   `qemu-system-aarch64` in the background → discover VNC via the QEMU
   monitor socket. The VNC password is always `testanyware`. A guest that
   needs a TPM (today, Windows) also gets the TPM state of its golden
   copied. If the golden has none, the guest gets a fresh, empty state.
   `vm start` also spawns an `swtpm` (TPM 2.0) and wires it to QEMU as a
   `tpm-tis-device`. A guest that does not need a TPM gets none of these.
   It needs no `swtpm` installed. `vm start` decides the TPM once, before
   it clones anything. It decides from what the QEMU binary of the host
   reports. The report comes from `qemu-system-aarch64 -machine none
   -device tpm-tis-device,help`. If that binary has no `tpm-tis-device`, the
   guest goes without a TPM. If `swtpm` is missing, `vm start` refuses the
   start with `SWTPM_MISSING`. Otherwise `vm start` attaches the TPM. Under
   HVF it attaches the TPM with `ppi=off` when the binary lists a `ppi`
   property. That property exists in QEMU 11.1+. Its PPI region aborts QEMU
   under Hypervisor.framework (ADR-0017).
4. Wait up to a boot budget (typically 120 s) for VNC to accept
   connections. On timeout, throw `VMLifecycleError.vncTimeout`.
5. Probe `GET /health` on the agent (port 8648) until it returns 200.
   If the agent never responds, `vm start` still writes the spec but
   omits the `agent` field. Callers can still drive via VNC.
6. Write `<id>.json` and `<id>.meta.json` atomically. Then print `<id>` on
   stdout. A QEMU-backed start also reports the TPM decision. In text mode
   it reports a line on stderr. In the `--json` envelope it reports a `tpm`
   object. The `--dry-run` plan also has a `tpm` object.

## Stop flow

1. Look up `<id>.meta.json` for the backend, clone dir, and viewer
   window id.
2. Send the backend-specific stop. For tart, run `tart stop <clone-id>`
   then `tart delete <clone-id>`. For QEMU, send QMP quit, then SIGKILL as
   needed.
3. Close the viewer window if one was opened (AppleScript).
4. Remove the clone dir (QEMU) and both JSON files.

## Golden images

Each golden image needs one build. Then every `vm start` clones from a
golden. All three goldens (macOS, Linux, Windows) are in-process
`testanyware vm create-golden --platform <os>` commands — no golden
shell scripts ship any more:

| Builder | Produces |
|---------|----------|
| `testanyware vm create-golden --platform macos` | `testanyware-golden-macos-27` (tart). `--version tahoe` builds `testanyware-golden-macos-tahoe`. |
| `testanyware vm create-golden --platform linux` | `testanyware-golden-linux-24.04` (tart) |
| `testanyware vm create-golden --platform windows` | `testanyware-golden-windows-11` (QEMU+swtpm, under `$XDG_DATA_HOME/testanyware/golden/`). The first run needs `--iso <path>`. |

Clone semantics: tart uses copy-on-write clones (`tart clone` is fast
and cheap). QEMU uses file-level copy of the backing qcow2 image — one
clone dir per running VM, removed on stop.

The macOS golden creation command temporarily disables SIP (via a
Recovery-boot csrutil cycle). It does this to install a TCC grant for the
agent with code-signing attached. The command re-enables SIP before it
finalises the image. See `testanyware vm create-golden --platform macos`
(ADR-0007/0008).

## Multiple concurrent VMs

Any number of VMs can run in parallel — each has its own id and its
own spec file. Every subcommand resolves which VM to target via
`--vm <id>` or `TESTANYWARE_VM_ID`. See
`docs/user/multi-vm-networking.md` for the cookbook.
