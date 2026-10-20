# cli-app-agent-routing-k24

## Goal

Add the client side of the protocol to the Host CLI. When `TESTANYWARE_APP_SOCKET` is present, route every agent connection through a guest stream. Add the test harness that drives the real CLI against the app.

## Context

- `docs/specs/testanyware-app.md`, sections "The protocol", "New error codes" and the first three requirements.
- `agent-client-hyper-k7`, which gives the connector seam.
- `docs/architecture/cli-design-contract.md`, for the error envelope and the exit codes.
- The commands `agent`, `file`, `window` and `menu-bar`, and the resolution of `--vm`, `--agent` and `TESTANYWARE_VM_ID` in the CLI resolve module.

## Done when

- The CLI checks the owner and the mode of the socket directory before it connects. It refuses an unsafe directory with `APP_UNREACHABLE` and cause `unsafe_socket_directory`.
- The CLI sends its protocol range, its build and its resolved directories with each request. It maps an error object to the envelope with the exit code that the code has today.
- With the variable set, each command that uses the agent client reaches the guest through `GET /v1/vms/{id}/agent`, or through `/v1/endpoints/...` for an explicit `--agent` endpoint. The CLI dials nothing itself.
- When nothing answers on the socket, the command fails with `APP_UNREACHABLE` and cause `not_running`. It makes no direct connection.
- Without the variable, the CLI behaves as before.
- A harness in the CLI test directory starts the app in test mode with temporary directories and fake guests. It runs the real CLI binary.
- Tests show that `agent snapshot`, `agent exec`, `file upload` and `file download` through the app print the same stdout as the direct path. A large transfer passes.
- A test shows an explicit `--agent` endpoint, and one shows the missing app.
- Existing tests still pass.

## Notes

- Write the harness for reuse. The leaves `cli-app-vnc-routing-k25`, `app-vm-list-k13` and later leaves build on it.
- Add `APP_UNREACHABLE` and `APP_INCOMPATIBLE` to the error catalogue of the code. The documents come in `llm-instructions-and-docs-k35`.
- `app-protocol-core-k9` left the rules in the crate `testanyware-app-protocol`: `validate_socket_path` (103 bytes), `check_socket_dir`, `ClientHello::to_headers` and `AppError`. Call them. The Host CLI's refusal of a longer socket path is this leaf's to wire.
- A harness that spawns the app must not spawn from two threads at once. On macOS a concurrent spawn can hand a child a listener or a pipe that a sibling opened, and tests then hang or read stale sockets. `crates/testanyware-app/tests/protocol_core.rs` shows the lock that fixed it.
- Port the `keep-on-failure` live check of `scripts/live-start-checks.sh` here, if the fake guests give it a fault to inject. `agent-client-hyper-k7` broke the old fault (a dead `HTTP_PROXY`).

