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

- `app-guest-streams-k23` fixed the stream contract. Build the path with `StreamTarget::path(StreamKind)` from `testanyware-app-protocol`, which also holds the parser. Send `Connection: Upgrade`, `Upgrade: testanyware-stream` (`STREAM_UPGRADE_TOKEN`) and the hello headers. Use hyper's client `with_upgrades()` and `hyper::upgrade::on` to take the stream after the `101`. The CLI resolves any default port first, because the app requires `host:port`.
- An error before the upgrade is the JSON error object with an HTTP status. Read the `code`, not the status. The codes are `CONNECTION_REFUSED` (`details.errno`, `details.cause`), `CONNECTION_TIMEOUT`, `VM_NOT_FOUND`, `NO_CONNECTION_SPECIFIED`, `INVALID_ENDPOINT`, `IO_ERROR` and `USAGE_ERROR`. The spec lists the statuses.
- The state directory that the app reads is the one with `vms/{id}.json` under it, the `state_dir` of `VmPaths`. In the hello, send the directory that `VmPaths` resolves, as the CLI does for its own reads.
- `crates/testanyware-app/tests/guest_streams.rs` holds a raw upgrade client, a wiremock agent and a synthetic RFB server with counted bytes. Reuse their shape. A harness that spawns the app must start its fakes first, then spawn once.

## Decisions (running log)

- **A new crate, `testanyware-app-client`.** It holds `AppClient::open_stream(&StreamTarget, StreamKind)`, which returns a tokio byte stream after the `101`. The VNC leaf needs the same call, so it does not live in the agent connector. It depends on the shared protocol crate and on hyper, builds on every host, and has its Unix code behind `cfg(unix)`. On a host without Unix sockets it fails with `APP_UNREACHABLE`. The `testanyware-cli` crate depends on it. The agent connector that adapts it to `AgentConnector` is `app_route.rs` in the CLI.
- **How a structured error crosses the connector seam.** `AgentConnector::connect` returns `io::Result`, which has no room for a code. `testanyware-agent-client` gained `ConnectorFailure` (code, message, remediation, details), which converts into an `io::Error`. The client unwraps it into `AgentError::Connector`. `AgentError::code()` now returns `&str`, because a connector's code is not static. The agent client knows nothing of the app. The CLI prints the failure in its own envelope with `exit_code_for(code)`.
- **The remedy.** The app leaves `remediation` empty for the codes that the Host CLI already explains (`VM_NOT_FOUND`, `CONNECTION_REFUSED` and the others). The CLI uses the app's remedy when there is one, and the remedy that the code has today when there is none. A mutation that removed the fallback failed a test. The spec says so now.
- **Two causes beyond the spec.** A socket path over 103 bytes is `APP_UNREACHABLE` with cause `socket_path_too_long`. Any socket failure other than a missing or refusing socket is cause `socket_error` with the errno. The spec listed three causes and no failure for these two. The table of the spec lists five now. `quitting` is a connection that the app closes before it answers, and a test shows it.
- **The target of a command.** `resolve_agent_target_with_env` keeps the order of the direct resolve: `--connect`, then `--vm`, then `--agent`. A `--vm` id stays an id, and the Host CLI reads no spec. `--connect` and `--agent` become an explicit endpoint, with the default port resolved. The `Host` header and the messages name a routed VM by its id. So the fake agent can tell a routed request from a direct one, and a test relies on it. An id that is not a plain id fails with `VM_NOT_FOUND` in the CLI. The path builder does not encode an id, and the app gives that answer anyway.
- **The hello.** The CLI states `SUPPORTED_RANGE`, its crate version and the directories of `VmPaths::from_process_env()`. I added `state_dir()` and `data_dir()` accessors to `VmPaths`. The `EnvProvider` of `resolve.rs` reads `TESTANYWARE_APP_SOCKET`, and an empty value counts as unset.
- **The catalogue.** `APP_UNREACHABLE` and `APP_INCOMPATIBLE` are in `ERROR_CODES` of `surface.rs`. Both exit 1 through the existing default. The documents come in `llm-instructions-and-docs-k35`, and I left a note there.
- **The harness.** `crates/testanyware-cli/tests/common/mod.rs` holds `serial()`, `FakeAgent` (wiremock), `TestApp` and the runners `cli`, `cli_direct` and `cli_with`. A test starts the fakes, then spawns the app once, and holds the global lock for its body. The harness builds the app executable itself with `cargo build -p testanyware-app`, because `cargo test -p testanyware-cli` does not. It takes the profile from the directory of the CLI binary. The CLI runs with a cleared environment, so no `TESTANYWARE_*` variable of the developer leaks in. `task app:routing-test` runs it.
- **A parity check does not prove routing.** A routed run that dials the guest itself prints the same output. When I made the CLI ignore the variable, the parity tests stayed green, and seven other tests went red. Two tests carry the proof. One shows that a routed `--vm` run names the agent by its id in the `Host` header. The other shows a run with the variable set and no app. The command fails with `APP_UNREACHABLE`, although the guest would answer a direct dial. The guest receives no request.
- **Controls seen to fail.** I broke the code in three ways and restored it each time. Routing ignored failed seven tests. No remedy fallback failed the envelope test. No directory check failed the unsafe-directory test, which holds a live listener in the unsafe directory. My first stress loop used a wrong binary path and reported 15 failures of 15, so it measured nothing. The corrected loop gave 0 failures in 15 runs of the exact test binary.
- **Cross builds.** `cargo zigbuild` passes for `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` for the app client, the agent client, the protocol crate and the vm crate. The whole CLI does not cross-build here, because `ffmpeg-sys-next` needs pkg-config. That is old, and no change of this leaf causes it.
- **The `keep-on-failure` live check is not ported.** It starts a real guest, and the fake guests are agents with no VM. `vm start` also stays direct until `app-vm-start-k27`. I left a note there about the hook that the check needs.
- **Source for the hyper API.** I read `hyper-1.9.0` in the cargo registry for `client::conn::http1::Connection::with_upgrades` and for `upgrade::on` with `Upgraded`. The code cites https://docs.rs/hyper/1.9.0/hyper/upgrade/index.html beside the call. `hyper-util` 0.1.20 supplies `TokioIo`, which wraps `Upgraded` as a tokio stream. The tests run the real app and the real hyper on both sides, and they pass.
- **Review.** I spent no in-session reviewer. Unit tests cover the pure rules. The integration tests cover each refusal and each parity with the real binaries. The mutations show that the tests can fail.
- **Not done here.** The VNC commands stay direct. That is `cli-app-vnc-routing-k25`, and I left notes there.
