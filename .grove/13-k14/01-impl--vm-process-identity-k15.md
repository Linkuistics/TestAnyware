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
