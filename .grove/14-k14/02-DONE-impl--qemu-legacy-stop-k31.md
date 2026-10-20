# qemu-legacy-stop-k31

## Goal

Stop an older QEMU guest through its control sockets, and never by a signal. This leaf comes before the signal guard of `app-adoption-k30`, so that the guard never leaves an older guest without a safe stop.

## Context

- `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", the last list.
- `vm-broker-design-k6`, finding F3.
- The QEMU monitor code and the TPM code of the vm crate.
- `vm-process-identity-k15`, which defines an older meta: a meta with no recorded identity.
- The stop of `app-vm-stop-delete-k28`. For a QEMU guest it calls the shared teardown, which signals the process and removes the clone and session directories without proof that QEMU stopped.

## Done when

- The app tells an older QEMU meta from a meta with identity by the absence of the identity fields. It needs no other fact from `app-adoption-k30`.
- For an older QEMU meta, `vm stop` in the app does not call the shared teardown. It sends no signal to any process.
- When the monitor answers, the app sends `quit`. An orderly exit of QEMU also ends its swtpm.
- When the control socket of swtpm still answers after QEMU is gone, the app sends the shutdown command on that socket.
- The app finds the session directory of an older VM by its id. It looks in its own temporary directory first, and then in `/tmp`.
- The stop succeeds when neither socket answers and no process holds the clone's disk, within a bounded wait. The app then removes the clone, the session directory and both sidecars.
- Otherwise the app removes nothing. The stop fails with `VM_STOP_FAILED`, and the details name what still answers or holds the disk. The remedy says to quit that QEMU by hand and run `vm stop` again.
- A QEMU meta with identity keeps the stop it has until `app-adoption-k30`. This leaf does not change that path.
- Tests use injected socket and disk probes. They cover the scenario "An older QEMU guest that does not stop", and they show that no signal is sent in either outcome.

## Notes

- A guest that a starter with another `TMPDIR` started is outside this leaf. The spec states the limit.
- The direct path of the Host CLI on a Linux or Windows host keeps its stop. Only the app uses this stop.

## Decisions (running log)

- **An older QEMU meta is a QEMU meta with no `identity`, `keeper_identity` and `swtpm_identity`.** `is_older_qemu_meta` in `legacy_stop.rs` decides it from the meta alone, so this leaf needs nothing from `app-adoption-k30`. A meta with any identity keeps `VmLifecycle::stop_with`.
- **The stop is a new entry point, `VmLifecycle::stop_with_probes`, and only the app calls it.** The direct path of the Host CLI on Linux and Windows still calls `stop`. The entry point dispatches on the meta, so the app loads no meta of its own.
- **Every outside fact goes through `LegacyStopProbes`.** The decision, `stop_through_sockets`, takes the answers of the probes and is pure. `SystemProbes` connects to the sockets, writes `quit` on the monitor, sends `CMD_SHUTDOWN` through the existing `swtpm::shutdown`, and lists the disk holders with `lsof -t` over the clone's qcow2 files. The trait has no signal operation, so no code path of the stop can send one.
- **The order is the safety of the stop.** Send `quit` when the monitor answers. Wait until the monitor is silent and no process holds the disk, which means QEMU is gone. Only then send the shutdown command to swtpm if it still answers. A running guest keeps its TPM. The stop succeeds only when a last look finds both sockets silent and the disk free.
- **"Answers" means that a connection is accepted.** A socket file with no listener refuses the connection, and so does a regular file. The disk check is the ground truth when the monitor is silent, because QEMU keeps its disk open until it ends. A host that cannot run `lsof` proves nothing, so the stop fails with `disk_check_error` and removes nothing.
- **The session directory comes from the first of these that exists:** the meta's `session_dir`, the app's own temporary directory, `/tmp`. A guest that a starter with another `TMPDIR` started has no findable session. The disk check still guards it, as the spec says.
- **The clone directory is the one in the meta, or the one derived from the id.** An older meta with no `clone_dir` used to fail the stop and clear the records. It now stops through the same checks, and succeeds when nothing holds a disk.
- **The wait is bounded by real time.** `SettleWait` has `attempts`, `interval` and `limit`. The first measurement counted only the pauses, and the check of the disk added about 0.5 s a look, so a held disk took 20 s. The limit counts the time of the looks: 10 s for QEMU and 10 s for swtpm, measured at 10.3 s for a held disk. A test shows that the wait ends at its limit however slow a look is.
- **The error is a new variant, `VmError::VmStopBlocked`, with the code `VM_STOP_FAILED`.** Its details name `monitor_socket`, `swtpm_socket`, `disk_holders` and `disk_check_error`, and its remedy says to quit that QEMU by hand and run `vm stop` again. It is boxed, so `VmError` keeps its size. The Host CLI agrees on the exit code, and a test covers that.
- **The routing tests that pinned the signal now use a meta with an identity.** Those tests describe "a meta with identity keeps its stop". New routing tests cover the older meta through the real CLI and the real app: a guest that is gone, a real monitor listener that receives `quit`, and a real process that holds the disk. Each shows that the process at the meta's pid is not signalled. The monitor test uses a short VM id, because the fixture's session path is otherwise longer than the 104 bytes of `sun_path`.
- **Docs.** The spec says in the protocol section that the older QEMU guest is an exception to "removes the sidecars on failure". The error-code references list the new details.
- **No in-session reviewer.** Unit tests, app tests and routing tests with real processes cover each claim, including that nothing is signalled.
