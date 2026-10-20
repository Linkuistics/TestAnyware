# cli-app-vnc-routing-k25

## Goal

Route every VNC connection through a guest stream when `TESTANYWARE_APP_SOCKET` is set. This covers `screen`, `input`, `screen record` and `viewer`.

## Context

- `docs/specs/testanyware-app.md`, section "The protocol".
- ADR-0005, ADR-0010, ADR-0016 and ADR-0018.
- The function `connect_vnc` and the viewer connect in the CLI. Both name `TcpStream` in their types today. The RFB client is already generic over its transport.
- The harness of `cli-app-agent-routing-k24`.

## Done when

- One boxed or enum stream type replaces `TcpStream` in the types of the CLI. The RFB client runs on a guest stream as it runs on TCP.
- With the variable set, `screen capture`, `screen size`, `screen find-text`, `input *` and `screen record` reach the guest through `GET /v1/vms/{id}/vnc`, or through `/v1/endpoints/...` for `--vnc` and `--connect`.
- `viewer` and `vm start --viewer` connect through a guest stream. The RFB work stays apart from the window (ADR-0005).
- The HiDPI logical target of the spec still applies (ADR-0016).
- A test with a synthetic RFB server shows a capture, a key press and a recording through the app. The outputs match the direct outputs.
- Without the variable, nothing changes.

## Notes

- The VNC keeper (ADR-0018) holds one silent connection and carries no traffic. It stays outside this leaf.
- After this leaf, no agent or VNC connection of the Host CLI is direct when the variable is set. Golden creation stays direct on every host.
- `app-guest-streams-k23` has the synthetic RFB server (`FakeRfb` in `crates/testanyware-app/tests/guest_streams.rs`). It speaks the version, the security type None and `ServerInit`, then moves counted bytes. A capture, a key press and a recording need real framebuffer messages on top of it.
- The app relays a half-close, so an RFB client that shuts down its write half still reads the rest of the guest's bytes.
- `cli-app-agent-routing-k24` left the client. `testanyware-app-client` has `AppClient::open_stream(&StreamTarget, StreamKind::Vnc)`, which checks the socket and its directory, states the hello and returns a tokio byte stream after the `101`. `app_route::client_hello` builds the hello, and `failure_of` turns an `AppError` into the `ConnectorFailure` that `exit_agent_error` prints. Reuse them.
- `resolve_agent_target_with_env` in `resolve.rs` is the agent twin you need. It leaves a `--vm` id as an id, so the Host CLI reads no spec for the agent. A VNC connection also needs the password and the HiDPI `logical` target of the spec. Read the spec for those two values, and still route by id, so that the Host CLI dials nothing.
- The harness is `crates/testanyware-cli/tests/common/mod.rs`: `serial()`, `FakeAgent`, `TestApp::start()` and the runners `cli`, `cli_direct` and `cli_with`. Add the synthetic RFB server to it, and let `add_vm` take a VNC address. Today `add_vm` writes a placeholder `vnc` section, because the app reads the spec as `VmSpec`, which requires one.
- `tests/app-agent-routing.rs` shows how to tell a routed run from a direct one. A routed `--vm` command names the agent by its id in the `Host` header, because the Host CLI never read the spec. A parity check alone cannot tell, because a direct dial prints the same output.
