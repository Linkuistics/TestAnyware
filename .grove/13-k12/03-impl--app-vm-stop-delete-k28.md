# app-vm-stop-delete-k28

## Goal

Run `vm stop` and `vm delete` in the app.

## Context

- `docs/specs/testanyware-app.md`, section "The protocol".
- The stop and delete code of the vm crate and the CLI.
- The harness of `cli-app-agent-routing-k24`.

## Done when

- `POST /v1/vms/{id}/stop` and `POST /v1/goldens/{name}/delete` return the results that the commands print today.
- With the routing on, the two commands ask the app. The text, the `--json` fields and the exit codes match the direct commands.
- A stop signals the same processes as the direct command does today. The identity guard comes later.
- A test with injected process control shows each outcome, including a missing VM.

## Notes

- After this leaf, every lifecycle command except golden creation goes through the app on a macOS host.

## Handed over by app-vm-start-k27

- `VmLifecycle::stop_with(id, paths, tools)` already stops a VM with the app's tool search, and `TartRunner::stop_in` takes the search too. `stop` delegates to it. The start uses `stop_with` when its client left as it succeeded.
- A start and a stop of one id should not overlap. The app serialises starts per id with `StartGate` (`testanyware-app/src/gate_start.rs`). A stop of an id that is starting is not gated yet.
- The detached children are reaped by threads of `detached::spawn_detached`, and `child_alive` still works beside them (`ECHILD` falls back to the signal-0 probe).
- Live checks that stop a VM by signal or `vm stop` see the app's cleanup a moment after the CLI has gone. `assert_vm_gone` in `scripts/live-start-checks.sh` polls for it.
