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

The meta also records the identity of each process that it names: the `pid`, the
keeper and, for a QEMU guest with a TPM, the `swtpm_pid`. The identity of a process
is its start time and the UUID of the boot session (`identity`, `keeper_identity`
and `swtpm_identity`). A recycled pid has another start time, so a reader signals a
recorded pid only when the identity matches. The meta of a QEMU guest also records
its `session_dir`, which holds the monitor socket and the swtpm control socket.
A meta written before these fields exist has none of them, and a reader still
accepts it.

## XDG paths

Resolved by the path helpers in `cli-rs/crates/testanyware-vm/src/paths.rs`.
Defaults follow the XDG Base Directory spec.

| Content | Path | Purpose |
|---------|------|---------|
| Running-VM specs | `${XDG_STATE_HOME:-~/.local/state}/testanyware/vms/<id>.json` and `<id>.meta.json` | Ephemeral. `vm start` creates them and `vm stop` removes them |
| Start records | `${XDG_STATE_HOME:-~/.local/state}/testanyware/vms/<id>.start-record` | Ephemeral. The TestAnyware app writes one before the clone of each start and removes it when the start ends. After a crash it stays until the app decides for it (`docs/specs/admission-queue.md`) |
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
   port, and dynamically generated password from the URL. Then spawn the
   VNC keeper (see [VNC keeper](#vnc-keeper)).
3. **QEMU path:** copy `${XDG_DATA_HOME}/testanyware/golden/<name>/` into
   a fresh `${XDG_DATA_HOME}/testanyware/clones/<id>/` → spawn
   `qemu-system-aarch64` in the background → discover VNC via the QEMU
   monitor socket. The VNC password is always `testanyware`. A guest that
   needs a TPM (today, Windows) also gets the TPM state of its golden
   copied. If the golden has none, the guest gets a fresh, empty state.
   `vm start` also spawns an `swtpm` (TPM 2.0) and wires it to QEMU as a
   `tpm-tis-device`. A guest that does not need a TPM gets none of these.
   swtpm ends when its QEMU ends: `vm start` gives its control socket the
   `terminate` option, which ends swtpm when the control connection closes.
   A start that fails before QEMU connects stops swtpm with a shutdown command
   on that socket.
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
2. Compare each recorded process with the live one. The meta records the
   pid, the start time and the boot session of the VM, the keeper and swtpm.
   A process receives a signal only on a full match. When the pid of the VM
   names another process, the stop signals nothing and removes nothing. It
   fails with `VM_STOP_FAILED`.
3. Send the backend-specific stop. For tart, run `tart stop <clone-id>`
   then `tart delete <clone-id>`, then signal the VNC keeper if it still
   runs and matches. For QEMU, signal the matching QEMU and swtpm.
4. Close the viewer window if one was opened (AppleScript).
5. Remove the clone dir (QEMU) and both JSON files.

A meta with no identity comes from an older release. The Host CLI on a Linux
or Windows host stops it as before. The TestAnyware app signals no process of
it. The app stops a tart guest by name and a QEMU guest through its monitor and
swtpm sockets.

## Adoption

When the TestAnyware app starts, it reads each meta and decides for each VM.
It adopts a VM whose main process matches its record. It reports a VM whose
process is gone as stopped. It reports a VM whose pid names another process as
not adopted, and it signals nothing. `GET /v1/status` and the status item list
each VM that the app did not adopt, with the reason.

## VNC keeper

On a macOS 27 host, Apple's VNC server aborts tart when a client connects 60
seconds or more after the last client left. Every `screen` and `input` command
opens its own VNC connection, so a command after a long pause can stop the guest.
ADR-0018 decides the remedy.

Each tart-backed `vm start` spawns a VNC keeper right after tart prints its VNC
URL. The keeper is the `testanyware` binary run as the hidden command
`_vnc-keeper --id <id> --host <host> --port <port>`. It runs detached, in its own
session. It holds one TCP connection to the VNC port, sends nothing, and reads
and discards what the server sends. The meta file records its pid as
`keeper_pid`. Its log is `<id>.keeper.log` in the same directory as the spec file.

The keeper stops itself. When its connection ends, it waits about one second and
connects again. When the port refuses, tart is gone, and the keeper exits. It also
exits when five connections in a row end within five seconds of opening.
`vm stop` signals the recorded pid as a backstop, only if the command line of that
pid still names `_vnc-keeper` and this id. Nothing restarts a keeper that dies.

QEMU guests, golden creation and `vm start --dry-run` start no keeper.

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
