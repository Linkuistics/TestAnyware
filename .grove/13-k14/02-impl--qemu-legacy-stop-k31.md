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
