# qemu-legacy-stop-k31

## Goal

Stop an older QEMU guest through its control sockets, and never by a signal.

## Context

- `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", the last list.
- `vm-broker-design-k6`, finding F3.
- The QEMU monitor code and the TPM code of the vm crate.

## Done when

- When the monitor answers, the app sends `quit`. An orderly exit of QEMU also ends its swtpm.
- When the control socket of swtpm still answers after QEMU is gone, the app sends the shutdown command on that socket.
- The app finds the session directory of an older VM by its id. It looks in its own temporary directory first, and then in `/tmp`.
- The stop succeeds when neither socket answers and no process holds the clone's disk, within a bounded wait. The app then removes the clone, the session directory and both sidecars.
- Otherwise the app removes nothing. The stop fails with `VM_STOP_FAILED`, and the details name what still answers or holds the disk. The remedy says to quit that QEMU by hand and run `vm stop` again.
- Tests use injected socket and disk probes. They cover the scenario "An older QEMU guest that does not stop".

## Notes

- A guest that a starter with another `TMPDIR` started is outside this leaf. The spec states the limit.
