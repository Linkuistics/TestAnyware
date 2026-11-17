# host-cli-control-k222

## Goal

The Host CLI cancels a guest command on request and on its own interruption.
It rejects a download body of the wrong length and removes its temporary file.
This leaf is candidate D.

## Context

- Spec: sections "Host CLI semantics", "Evidence transfer", "Compatibility",
  "Bounds" and candidate D.
- ADR-0025 and ADR-0026.
- Crates: `cli-rs/crates/testanyware-protocol`, `testanyware-cli`,
  `testanyware-agent-client` (tests `endpoint_integration.rs` and
  `transport_seam.rs`) and `testanyware-app`.
- Old failing paths: the CLI SIGTERM case of retired leaf 40, the interrupt
  case of retired leaf 50 and the recorded old Linux download reply.

## Done when

- The protocol crate holds the cancel types.
- `file cancel` with `--exec-id` cancels a running command. An old agent
  replies 404, and the Host CLI reports that the agent cannot cancel.
- On SIGTERM, the Host CLI sends a cancel before it exits. Its own exit
  follows within the spec's bounds of 2 s, then SIGKILL after 2 s more.
- The download framing check rejects an extra or a short body. Tests in
  `testanyware-agent-client` send raw HTTP that includes the recorded old
  Linux reply.
- The Host CLI removes its temporary download file on SIGTERM.
- `app:routing-test` covers the app route for `file cancel`.
- Host CLI tests run each case against a loopback agent.
- Text mode does not change for an existing command.
- `cli:test`, `app:routing-test`, `cli:clippy` and `cli:fmt` pass.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

Do not patch the global CLI or app. Test against private builds only.
