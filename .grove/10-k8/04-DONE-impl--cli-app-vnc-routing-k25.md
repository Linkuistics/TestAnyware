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

## Decisions (running log)

- **One boxed stream type.** `VncStream` in `commands/mod.rs` is a trait object over `AsyncRead + AsyncWrite + Unpin + Send`. `VncConnection` is `RfbConnection<BufReader<Box<dyn VncStream>>>`. `input.rs` and `connect_vnc` name it, and no type of the CLI names `TcpStream` for RFB now. I chose a box over an enum, because the app stream is hyper's `Upgraded` wrapped in `TokioIo`, and a box needs no match arm per transport. The direct path opens its own `TcpStream`, sets `nodelay` and runs `RfbConnection::handshake`, which is what `RfbConnection::connect` does.
- **The resolver.** `resolve_vnc_endpoint_with_env` in `resolve.rs` first runs the direct `resolve_vnc_with_env`, so the CLI reads the spec of a `--vm` id for the password and the HiDPI `logical` target. Then, with the variable set, it adds a route. A `--vm` id routes as the id, and `--connect` and `--vnc` route as an explicit endpoint, in the order of the direct resolve. The agent twin reads no spec. This one must, and it still dials nothing.
- **Errors.** `connect_vnc` returns `VncConnectError`: an `RfbError`, or the `AppError` of a refused stream. `exit_vnc_connect_error` prints the first as before and the second through `exit_connector_failure`, which `exit_agent_error` now shares. So the app's remedy wins and the remedy of the code is the fallback, for both families. A dial that the app refuses comes before the `101`, so the codes are the ones of the direct path (`CONNECTION_REFUSED` and the others).
- **The viewer.** `connect_and_stream` calls `connect_vnc` inside its existing `select!`, so a window close still cancels a connect. `connect_vnc` applies the logical target, so the viewer no longer applies it. `vm start --viewer` calls `run_viewer`, so it routes with it. No test opens a window. The shared `connect_vnc` is what the routing tests cover.
- **The harness.** `FakeRfb` in `tests/common/mod.rs` is a synthetic RFB 3.8 server. It offers None, or VNC authentication when it has a password. It answers each update request with the whole frame as one Raw rectangle, and it records key and pointer events. `add_vnc_vm` writes a spec with the server's address, its password and a `logical` target. `connect_spec` writes a spec for `--connect`. `add_vm` is unchanged.
- **What the tests show.** `screen size`, `screen capture` (byte-equal PNG), `input key`, `input click`, an explicit `--vnc` endpoint, a `--connect` spec and `screen record` give the direct output through the app. A `@2x` spec still gives `32x24` and doubles a click. The spec's password reaches the handshake, and so does `TESTANYWARE_VNC_PASSWORD` for `--vnc`. A wrong password is `AUTH_REQUIRED` with exit 4 on both paths. A refused guest is `CONNECTION_REFUSED`. An unknown id is `VM_NOT_FOUND` and dials nothing.
- **A parity check does not prove routing, again.** When `connect_vnc` ignored its route, 14 of 15 tests stayed green. One test failed: with the variable set and no app, the commands fail with `APP_UNREACHABLE` and the guest accepts no connection. That test carries the proof, as in `cli-app-agent-routing-k24`. No `Host` header exists on RFB, so no second proof was possible here.
- **Controls seen to fail.** I broke the code in three ways and restored it each time. Ignoring the route failed the no-app test. Skipping the logical target on the routed path failed the HiDPI test. Dropping the password on the routed path failed two password tests. My first stress loop took its binary path from `ls`, which is an alias here. It reported 15 failures of 15, and the code had no part in them. The corrected loop ran the exact test binary 40 times with 0 failures.
- **A defect that is not this leaf's.** The stress loop found `screen record` failing once in 25 runs with `server sent unexpected message type 240`. The recorder races `next_message` against its timers in a `select!`, and a cancelled read loses its bytes. I showed that the direct path fails the same way when the server writes an update in 1500-byte pieces with 3 ms pauses. So the defect is old, and the app only adds a hop. I cut `rfb-cancel-safe-reads-k41` ahead of `app-bundle-k10`, and left the recording test with a 16 by 16 frame (one segment per update). The test says why in a comment.
- **Review.** I spent no in-session reviewer. The unit tests cover the resolver. The integration tests cover each refusal and each parity with the real binaries, and the mutations show that they can fail.
- **Cross builds.** The leaf changes only the CLI crate, which does not cross-build here (`ffmpeg-sys-next` needs pkg-config, as `cli-app-agent-routing-k24` found). The CLI code has no `cfg` for a host, so Linux and Windows compile the same types.
