# harness-golden-forward-k51

## Goal

The harness forward stops dialling the macOS golden. For each accepted connection it opens a guest stream by the golden's id, through the app. After this leaf, `linux_host_harness` passes from an agent session that has no Local Network access of its own. This is the demonstration of `harness-guest-streams-k47`, and it unblocks `non-macos-runtime-check-k40`.

## Context

- The brief of `harness-guest-streams-k47`, Notes. The decision log of `app-all-guest-traffic-k42` holds the reasons: "The harness forward stays in the test process, and only its dial moves into the app" and "The forward keeps the one-user rule as far as a TCP listener can".
- TN3179 says that accepting a connection needs no Local Network access. Only an outgoing dial does.
- `harness-golden-hut-k49` added the public `app_route` function that opens a guest stream for a target and a kind. Use it. The harness builds no path and no header.
- The forward today is `PortForward` with `forward_accept_loop` and `forward_splice`. It binds `0.0.0.0:0` and dials the golden's agent and VNC endpoints from the spec.
- The Windows harness has its own copy of the forward. `remaining-direct-dials-k48` changes it. Do not touch it here.

## Done when

- `forward_splice` opens a guest stream by the golden's id, with the agent kind or the VNC kind, instead of `TcpStream::connect`. The harness reads the golden's spec only for the VNC password, as the Host CLI does for `--vm`.
- The forward binds the HUT's gateway address, not `0.0.0.0`. It accepts only the HUT's own address, the `src` of `ip route show default`. A connection from any other peer is closed.
- The address choice and the peer check are pure functions with unit tests. Tests cover the gateway address, the HUT's address, a foreign peer and a missing route.
- Demonstration: `linux_host_harness` passes from an agent session that has no Local Network access of its own. A person allowed the development bundle of the app. The binary is a workspace build for aarch64 Linux. The decision log records the command, the result line and the list of passed checks.
- If the session cannot run the harness, the log says what is missing. It does not claim the demonstration.
- The module docs of `linux-host-harness.rs` describe the forward, the bind address and the peer check as they now are. The text on running the test from Terminal is gone, because the test no longer needs Terminal.
- `task release:linux-harness` still parses. Its description matches the new run.

## Notes

- The Linux HUT is a tart guest, so the gateway and the peer address are right. `harness-golden-hut-k49` made the backend check fail for any other backend.
- The forward lives only for one harness run.
- Do not add a flag, an exit code or a `--json` field to the Host CLI.
