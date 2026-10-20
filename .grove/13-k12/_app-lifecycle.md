# app-lifecycle-k12 — brief

## Goal

Run `vm list`, `vm start`, `vm stop` and `vm delete` in the app. An agent under herdr can then start, drive and stop a macOS guest. This is the increment that ends the herdr failure of issue #8.

## Done when

- On a macOS host, the four lifecycle commands go through the app. The output, the `--json` fields and the exit codes match the direct commands.
- The app starts `tart run`, QEMU, swtpm and the VNC keeper detached in their own sessions. It reaps each child that exits. Each child outlives the app.
- The app starts each VNC keeper from its own executable.
- A client that closes its connection during `vm start` cancels the start. The app cleans up as after a failed start, and `--wait` ends its wait for a guest slot.
- A denial in the health wait of `vm start` gives `VM_BOOT_TIMEOUT`, cause `local_network_denied`, and a remedy that names the TestAnyware app.
- The herdr acceptance run passes on the development bundle.

## Decomposition

1. `app-vm-list`: the lifecycle request shape, the app's use of the vm crate, and the first command.
2. `app-vm-start`: the start with progress events, cancellation and the diagnosis.
3. `app-vm-stop-delete`: the last two commands.
4. `herdr-acceptance-run`: the live check from herdr.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, sections "The protocol", "Lifecycle and adoption" and "The Local Network diagnosis".
- ADR-0004, ADR-0009, ADR-0010, ADR-0017, ADR-0018 and ADR-0019.
- Glossary terms: VNC keeper, macOS guest limit, Host-side framebuffer.
- Test seams: the harness of `app-streams-k8` and the existing injected probes of the vm crate.

## Notes

- The process identity records and adoption come in `app-adoption-k14`. In this increment, `vm stop` signals as the direct command does today.
- The app has no shell environment. Give it the built-in tool search of the spec: `/opt/homebrew/bin`, `/usr/local/bin` and `/usr/bin`.
- A request carries the options that the CLI parsed and the directories that it resolved. The app reads nothing from a client environment.
