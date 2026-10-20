# app-vm-start-k27

## Goal

Run `vm start` in the app. The CLI sends the options and prints the progress and the result as it does today.

## Context

- `docs/specs/testanyware-app.md`, sections "The protocol", "Lifecycle and adoption" and "The Local Network diagnosis".
- ADR-0017, ADR-0018 and ADR-0019.
- The start, the health wait and the keeper code of the vm crate.
- The amendment of the CLI contract for `vm start --wait`, which writes progress to stderr.

## Done when

- `POST /v1/vms` answers with progress events as NDJSON. A result event or an error event ends the stream.
- The CLI prints the progress lines to stderr as today, and it prints the result with the same stdout and exit code. `--quiet` and `--json` keep their meaning.
- The app spawns `tart run`, QEMU, swtpm and the keeper detached, each in its own session. The keeper runs from the app's own executable.
- The app reaps each child that exits. A test shows that no zombie stays.
- When the client closes its connection before the result, the app cancels the start and cleans up as after a failed start. `--wait` ends its wait for a slot.
- The health wait keeps its rule of five `host_unreachable` probes and then the `nc` oracle. A denial ends the start with `VM_BOOT_TIMEOUT`, cause `local_network_denied`. The remedy names the TestAnyware app.
- Unit tests cover the cancellation and the denial with scripted probes.
- The leaf records one live start of a macOS guest through the app from Terminal.

## Notes

- Start `swtpm` as the direct path does today. The `terminate` option and the identity records come in `app-adoption-k14`.
- The direct path of Linux and Windows hosts keeps its remedy text for a denial. The text names the app only on a macOS host.
- The app accepts client sockets and spawns `tart`, QEMU and `swtpm` from several threads. On macOS a new socket gets close-on-exec in a second call, so a spawn on another thread can hand a child that socket. `app-protocol-core-k9` saw this in its tests. Spawn the detached children so that they hold no client socket, for example from one thread that owns the spawning, and test that a detached child holds no socket of the app.
- The `keep-on-failure` live check of `scripts/live-start-checks.sh` still has no fault. `cli-app-agent-routing-k24` could not port it: the check starts a real guest, and its fake guests are agents with no VM, while `vm start` stays direct until this leaf. A fault that starves the agent probes of a real start needs a hook on the app's start path, such as a test-mode argument. Add it here, and port the check.
