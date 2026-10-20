# app-streams-k8 — brief

## Goal

A headless TestAnyware app serves guest streams on a Unix socket, and the Host CLI routes every agent and VNC connection through it. This is the protocol seam of the design. Tests drive it with the real CLI binary and fake guests.

## Done when

- The app executable runs in a test mode. The mode has no status item, no login item and no LaunchServices. It takes a temporary socket, state directory, data directory and configuration path.
- The socket is safe. Its directory has mode 0700 and belongs to the user. The app refuses any other peer uid, a path of more than 103 bytes, and a second instance.
- A request with a protocol range that does not overlap fails with `APP_INCOMPATIBLE`, cause `protocol`. A request with other directories fails with cause `directories`.
- `GET /v1/vms/{id}/agent` and `/vnc`, and `GET /v1/endpoints/{host}:{port}/agent` and `/vnc`, give a guest stream after `101 Switching Protocols`. The bytes pass unchanged in both directions, and a half-close passes through.
- When `TESTANYWARE_APP_SOCKET` names the socket, every command that reaches a guest by agent or VNC uses a guest stream. The output matches the direct output.
- When that variable is not set, no command changes. The default on macOS stays direct until `app-bundle-k10` flips it.
- A test harness starts the app with a wiremock In-VM agent and a synthetic RFB server on loopback. It runs the real CLI binary against them.

## Decomposition

1. `app-protocol-core`: the socket, its safety checks, the status request and the handshake.
2. `app-guest-streams`: the relay, the dial rules and the Local Network diagnosis.
3. `cli-app-agent-routing`: the client side of the protocol, the agent commands and the harness.
4. `cli-app-vnc-routing`: the same for `screen`, `input`, `record` and `viewer`.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, sections "The socket and access", "The protocol", "Version skew" and "The Local Network diagnosis".
- ADR-0004 and ADR-0019.
- Glossary terms: TestAnyware app, Guest stream, Direct path, Local Network denial.
- Test seam: the protocol of the app, with the fakes named in the root brief. Pure functions and injected probes cover the decisions inside the app.

## Notes

- Keep the protocol types, the socket rules and the stream client in a crate that builds on every host. The Host CLI uses them on Linux and Windows too. Only the status item and the login item need AppKit, and they come in `app-bundle-k10`.
- The test variables `TESTANYWARE_APP_SOCKET` and `TESTANYWARE_CONFIG` exist for tests. The variable `TESTANYWARE_APP_SOCKET` is also what turns routing on in this increment.
- The remedy of a directory mismatch names the environment variable that differs. The leaf `config-in-app-k33` adds the file `config.rhai` to that remedy.
- The progress of lifecycle commands is not part of this increment. Lifecycle commands stay direct until `app-lifecycle-k12`.
