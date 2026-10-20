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
