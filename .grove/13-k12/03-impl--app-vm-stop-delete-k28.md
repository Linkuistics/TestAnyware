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
