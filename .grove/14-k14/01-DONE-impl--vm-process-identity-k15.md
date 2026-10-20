# vm-process-identity-k15

## Goal

Record the identity of each VM process in the meta. Start swtpm so that it ends when its QEMU ends.

## Context

- `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", the rules of adoption.
- `vm-broker-design-k6`, finding F3, which holds the swtpm option and its source.
- The meta and spec code of the vm crate, and the TPM code (ADR-0017).

## Done when

- The meta records the pid, the start time and the boot session UUID of each process that it names. A write is atomic.
- The meta of a QEMU guest records its session directory.
- swtpm starts with the option that ends it when its control connection closes. A start that fails before QEMU connects stops swtpm through its control socket.
- A reader still accepts an older meta that has none of these fields.
- Pure functions read the identity of a process through an injected probe. Unit tests cover them.
- The Host CLI and the app share this code. A test shows that a started VM has a complete meta.

## Notes

- The change applies to the direct path on every host. It must keep each existing `--json` field.

## Decisions (running log)

- **Identity is a start time and a boot session UUID, per process.** The meta gains `identity`, `keeper_identity`, `swtpm_pid`, `swtpm_identity` and `session_dir`, each omitted when absent. An older meta still loads with none of them. The start time has no common unit: microseconds since the epoch on macOS (`proc_pidinfo`, `PROC_PIDTBSDINFO`), clock ticks since boot on Linux (`/proc/<pid>/stat` field 22). A reader compares it for equality only. The boot session is `kern.bootsessionuuid` on macOS and `/proc/sys/kernel/random/boot_id` on Linux. Other hosts give no identity.
- **`read_identity` is pure over an injected `ProcessProbe`.** `SystemProbe` is the host probe. The comparison of a recorded identity with a live one belongs to `app-adoption-k30`, so this leaf adds none.
- **The identity is read right after the spawn,** before any wait: for `tart run` in `observe_tart_boot`, for the keeper beside `start_keeper`, and for QEMU and swtpm in `QemuRunner::start_in`. A cancelled tart start reads it from `StartTrack`.
- **swtpm starts with `--ctrl ...,terminate` and `--pid file=<session>/swtpm.pid`.** Source: swtpm 0.10.2 `swtpm(8)` and `tpm_ioctl.h` (`CMD_SHUTDOWN` = 3, a big-endian 32-bit request, a 32-bit result). A real swtpm confirmed three facts in `tests/swtpm.rs`: the pid file names the daemon, a closed control connection ends it, and `terminate` alone ends nothing before a first connection.
- **`qemu::teardown` sends `CMD_SHUTDOWN` before the `pgrep` backstop.** That stops a swtpm that no QEMU ever connected to. The backstop stays for a guest that an older start made. A test puts the TPM state outside the clone, so only the control socket can have stopped swtpm.
- **The Windows golden creation keeps its swtpm as it was.** The brief covers the direct `vm start` path only.
- **`StartArtifacts` carries `session_dir`, `identity`, `swtpm_pid` and `swtpm_identity`.** `qemu_meta` builds the meta from them, so a kept VM after a failed start carries the same record.
- **No in-session reviewer.** Tests cover each claim, including the real swtpm behaviour.
