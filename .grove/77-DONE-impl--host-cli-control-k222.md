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

## Decisions (running log)

- `file cancel` also takes `--dry-run`. Contract §9.3 requires the flag on
  each mutating command, and the contract test checks it.
- The Host CLI maps `exec_id_in_use` to `EXEC_ID_IN_USE` with exit 5. Exit 5
  is the conflict class of §5. Only HTTP 409 on `/exec` reads the error body,
  so every other exec failure keeps its code.
- The framing check uses hyper 1.9.0 `Connection::without_shutdown`. After a
  `Connection: close` exchange it returns the unread `read_buf` and the stream.
  A byte in either before end-of-file is `DOWNLOAD_FRAMING_INVALID`. The wait
  for end-of-file runs under the request deadline.
- A drop guard owns the temporary download file. A signal drops the download
  future, and the guard removes the file before the Host CLI raises the signal.
- The handler is installed only for `file exec` and `file download`, and only
  on Unix. A signal that the parent set to ignore stays ignored, so a
  background job keeps today's behaviour.
- The old-path control builds the Host CLI of revision `251bf072e3` from
  `git archive` in a temporary directory. It runs the new interruption and
  cleanup cases against that binary, and each case must fail.
- The download flushes the temp file with `sync_all` before the rename. The
  spec's step 4 says "flush to disk", and a user-space flush does not reach
  the disk.
- A reset or a connection error after a complete body fails the download
  with `CONNECTION_DROPPED`. The client cannot prove that nothing followed
  the body, so it never reports such a transfer as a success.
- The website copy `website/docs/reference/error-codes.md` stays unchanged.
  It already differs from the source copy by 64 diff lines, and it has 9 hard
  prose findings. The source copy and `LLM_INSTRUCTIONS.md` carry the codes.
- The old-path build archives `cli-rs`, `docs`, `provisioner` and
  `LLM_INSTRUCTIONS.md`. These are every path outside `cli-rs` that the
  crates embed. The harness reads `TESTANYWARE_CONTROL_CLI` only in that
  control, for both `cli` and `spawn_cli`.
- The control run of `task cli:control-old-path` passed: each of the three
  cases failed on the binary of `251bf072e3` with its known defect, and
  passed on this checkout.
