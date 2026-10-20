# app-vm-list-k13

## Goal

Link the lifecycle code of the vm crate into the app, and serve `vm list` through `GET /v1/vms`.

## Context

- `docs/specs/testanyware-app.md`, sections "The protocol" and "Lifecycle and adoption".
- The vm crate lifecycle module and the code of `vm list` in the CLI.
- The harness of `cli-app-agent-routing-k24`.

## Done when

- The app executable links the vm crate. It resolves tools with the built-in search, and it reads the state directory of its own configuration.
- `GET /v1/vms` returns the listing that `vm list` prints today, in text and in `--json`.
- With the routing on, `vm list` asks the app. Without it, the command is direct as before.
- A test with sidecars in a temporary state directory and an injected `tart list` shows the same output through the app as the direct output.
- The status item menu lists the running VMs.

## Notes

- A VM that the direct CLI started has the same sidecars. The app lists it, because the list is rebuilt at each request.
- Stop at the listing. The start and the stop have their own leaves.
