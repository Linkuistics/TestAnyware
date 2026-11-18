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
- At SIGINT or SIGTERM, `file exec` follows the spec's "CLI interruption"
  steps. It opens a new guest connection and sends one cancel with
  `waitMs: 0`. It waits 1.5 s at most for the reply.
- Then the Host CLI restores the default action and raises the same signal.
  The parent sees the same signal death as today, within the spec's 2 s
  bound.
- When an old agent replies 404 to that cancel, the Host CLI raises the
  signal without a cancellation.
- The 2 s, then 2 s, SIGTERM-and-SIGKILL bound belongs to the outer
  controller. It does not prove these steps.
- The download framing check rejects an extra or a short body. Tests in
  `testanyware-agent-client` send raw HTTP that includes the recorded old
  Linux reply.
- At SIGINT or SIGTERM, the Host CLI removes its temporary download file and
  raises the signal again.
- `app:routing-test` covers the app route for `file cancel`.
- Host CLI tests run each case against a loopback agent. The interruption
  and cleanup cases cover both signals and check the signal death.
- The CLI SIGTERM case of retired leaf 40 and the interrupt case of retired
  leaf 50 fail on the old path. Each new case passes.
- Text mode does not change for an existing command.
- `cli:test`, `app:routing-test`, `cli:clippy` and `cli:fmt` pass.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

Do not patch the global CLI or app. Test against private builds only.

Leaf linux-agent-cancellation-k220 added `/exec/cancel` to the Linux agent.
Its reply is `{"state": S}` or `{"state": "terminal", "record": R}`, as the
spec's "Cancel endpoint" table gives. The agent returns 409 with the new wire
token `exec_id_in_use` for a duplicate `id`. Map that token to a Host CLI
code here and add it to the §4.5 table of `cli-design-contract.md`. Then
remove it from `KNOWN_NON_CANONICAL_EXCEPTIONS` in
`agents/linux/tests/test_error_tokens.py`.

The recorded Linux exec fixtures
hold no cancelled record yet. The recorder `agents/linux/tests/record_exec_replies.py`
can add such cases, and the Rust tests count 14 Linux fixtures today.
