# host-cli-control-k239

**Reviews:** host-cli-control-k222

## Goal

Try to break the Host CLI change of host-cli-control-k222. The acceptance
leaves k223 to k225 drive `file exec`, `file cancel` and `file download`
through it next. Find each signal, cancellation or download that gives the
caller a wrong outcome. Find each wrong outcome that the caller cannot tell
from a correct one.

## Context

- Spec `docs/specs/guest-command-lifecycle.md`: sections "Exec id", "Cancel
  endpoint", "CLI interruption", "Host CLI semantics", "Evidence transfer",
  "Compatibility and version skew", "Bounds" and "Test seams and required
  controls".
- ADR-0025 and ADR-0026.
- The contract of k222 is its `Done when` list and its notes.
- The code:
  - `cli-rs/crates/testanyware-protocol/src/agent_requests.rs` and
    `exec_record.rs`: the cancel request, the exec id rule and the cancel
    reply reader.
  - `cli-rs/crates/testanyware-agent-client/src/lib.rs` and `transport.rs`:
    `cancel_exec`, the 409 path of `exec`, the download and its temp file.
  - `cli-rs/crates/testanyware-cli/src/commands/interrupt.rs`,
    `commands/file.rs`, `commands/mod.rs`, `main.rs`, `surface.rs`,
    `output.rs` and `discoverability.rs`.
- The controls:
  - `cli-rs/crates/testanyware-cli/tests/host-cli-control.rs` and the
    `file cancel` case in `tests/app-agent-routing.rs`.
  - `cli-rs/crates/testanyware-agent-client/tests/transport_seam.rs`,
    `tests/endpoint_integration.rs` and `src/streaming_tests.rs`.
  - `task cli:control-old-path`, with `scripts/cli-control-old-path.sh`.
- Documents: `docs/architecture/cli-design-contract.md`,
  `docs/architecture/agent-protocol.md`, `docs/reference/error-codes.md`,
  `LLM_INSTRUCTIONS.md`, `docs/reference/cli-schemas/file-cancel.json` and
  `file-exec.json`.
- The agents that answer these requests: linux-agent-cancellation-k220 and
  macos-agent-cancellation-k221, with their reviews.

## Specific doubts

- Signal handling: a signal can arrive before the listeners exist, between
  the select and the raise, or twice. Check that each order still ends in the
  same signal death within the spec's 2 s bound. Check the multi-threaded
  runtime: which thread raises, and whether any thread can block the death.
- The handler keeps a signal that the parent ignored. Check every other
  inherited disposition and the signal mask.
- The cancellation of an interrupted exec shares the client of the exec.
  Check that it opens a new guest connection on the normal route, also on
  the app route. Check that the 1.5 s limit covers the dial.
- The framing check waits for the agent to close the stream after the body.
  Find each agent, relay or HTTP version for which that wait can stall, give
  a false failure or miss bytes. Compare HTTP/1.0 and HTTP/1.1 replies, a
  reset after the body and bytes split across writes.
- The temp file guard removes the file when the future is dropped. Find each
  await point where a dropped future leaves a temp file. Find each point
  where the guard can remove a file that is now the destination.
- `file cancel` checks a terminal record against the cancelled id. Check
  every state with and without a record, a legacy record, and the 404 path
  through the app.
- Only HTTP 409 on `/exec` reads the error body. Check that every other
  existing exec error keeps its code, its exit and its text.
- Text mode, the existing JSON fields and the exit codes of each existing
  command must not change. Check `--detach`, `--dry-run` and the aliases.
- The old-path control builds an old revision from `git archive`. Judge
  whether each case fails for the stated defect and not for another reason.
  Every control must be able to fail on the new code.

## Done when

- Each finding names the code, the input and the observed or derived wrong
  result.
- The review reports findings. It changes no code.

## Notes

Review the producer commit of host-cli-control-k222 against the current
source. Report each defect, and leave each code change to the integration
step. Do not patch the global CLI or app. Test against private builds only.


## Decisions (running log)

- Review producer commit `282ee507f4036ae74fc41ab959cf9b66f86fd91d` against
  the current source by inspection only. No test, build, lint, format or
  runtime probe runs. The graph status and coverage calls both fail before
  serving a query because an unverified CBM generation is active. Direct
  source reads, including the locked dependency implementations, are the
  evidence; graph freshness and coverage are unavailable.
- The signal listeners stop being polled when the request completes, while
  Tokio keeps their process handlers registered. Record the completion race
  and blocked-output consequence as one signal-lifetime finding.
- The new synchronous temp-file creation and synchronous drop cleanup execute
  on the task that must observe a signal. A blocking filesystem operation can
  prevent signal handling. Record that separate local-I/O boundary.
- The framing check trusts Hyper's selected body framing without comparing
  the advertised length. Hyper's HTTP/1.1 decoder gives Transfer-Encoding
  priority over Content-Length, so contradictory length evidence can pass.
- On the app route, a guest read error ends the relay by dropping the client
  Unix socket. After an already relayed body this can appear as clean EOF to
  the new framing check. Record the route-dependent reset result separately.
- The old-path runner invokes a test function for each signal control. Each
  function tries SIGINT first and panics at its expected old-code defect,
  so it never reaches SIGTERM on that binary. Record the missing required
  SIGTERM evidence; do not infer that the recorded new-code run failed.
- The findings concern the existing signal, transfer and old-path acceptance
  conditions. One flat integration sibling before acceptance-plan-k223 can
  triage them. No new context node or duplicate adversarial review is needed.

## Findings

Reviewed source: producer commit
`282ee507f4036ae74fc41ab959cf9b66f86fd91d`, also the current source before this
review's notes. Every wrong result below is derived from source. None is a
new runtime measurement. This review changes no production or test code.

### R1 — P1: signals are swallowed after request completion

Code: `cli-rs/crates/testanyware-cli/src/commands/file.rs:41-49`,
`:95-124` and `:454-470`, with
`cli-rs/crates/testanyware-cli/src/commands/interrupt.rs:24-41`.

Input: run an attached exec that returns stdout larger than a pipe's
capacity, with the Host CLI's stdout connected to a parent that keeps its
read end open but stops reading. Let the agent reply finish, then send
SIGTERM or SIGINT while the Host CLI prints the result. The same condition
can be arranged on stderr. A second ordering makes both the response and a
signal ready at the final `until_interrupted` selection.

Derived wrong result: the signal listener is polled only during the request.
`run_exec` drops it before printing, and `run_download` retains it but never
polls it during printing. Tokio 1.52.1 explicitly keeps its process signal
handler installed after a `Signal` is dropped. A later signal therefore
only produces a notification that no code consumes. The blocked writer
stays alive past 2 s and needs SIGKILL. Without output backpressure it can
exit normally instead of dying of the signal. If both selection branches
are ready, Tokio's unbiased selection can choose `Done` and discard the
already received signal the same way.

This breaks the same-signal-death and 2 s contracts and changes the old
CLI's response to interruption during output. The new signal controls
interrupt a request deliberately held open; `spawn_cli` also discards
stdout, so they do not exercise either ordering.

### R2 — P2: synchronous temp-file creation can prevent interruption

Code: `cli-rs/crates/testanyware-agent-client/src/lib.rs:559-568` and
`:880-903`, reached under the signal selection in
`cli-rs/crates/testanyware-cli/src/commands/file.rs:454-459`.

Input: in a writable destination directory, place a FIFO with no reader
at `.evidence.json.testanyware-PID.0.tmp`, where PID is the Host CLI's PID,
then download into `evidence.json` from an agent that returns headers.
A launcher can create that exact entry using its own PID and then `exec`
the Host CLI, preserving the PID. Send SIGTERM or SIGINT after the headers.

Derived wrong result: `std::fs::File::create` opens the existing FIFO for
writing and waits for a reader. It does not use exclusive creation or
reject an existing non-regular entry. Unlike the former async creation,
this new open blocks inside a poll of the same task that must poll the
signal receiver. Other runtime threads cannot advance that task. The
installed signal handler catches the signal, and signal-hook-registry
1.4.8 installs it with `SA_RESTART`, so the open continues waiting. No
`TempFile` guard exists yet. The Host CLI neither removes the temp entry
nor raises the signal within 2 s.

This is a concrete local-file input that breaks interruption and cleanup,
not a request for a general guarantee against uninterruptible kernel I/O.
The dropped-future control only pauses an ordinary download while waiting
for more network bytes, after temp-file creation has returned.

### R3 — P2: a conflicting transfer encoding bypasses the length check

Code: `cli-rs/crates/testanyware-agent-client/src/lib.rs:591-609`,
`:910-944` and `:951-986`.

Input: serve these raw HTTP/1.1 bytes, then close normally:

```text
HTTP/1.1 200 OK\r\n
Content-Length: 5\r\n
Transfer-Encoding: chunked\r\n
\r\n
6\r\n
abcdef\r\n
0\r\n
\r\n
```

The displayed `\r\n` means the two wire bytes CR and LF; the extra display
line breaks are not wire bytes.

Derived wrong result: Hyper 1.9.0's `proto/h1/role.rs`, `Client::decoder`,
selects chunked decoding whenever Transfer-Encoding ends in `chunked`,
ahead of Content-Length. The client never independently validates the
length header against the decoded byte count or rejects that conflicting
framing. `stream_to_writer` writes all six bytes. The zero chunk ends
Hyper's body, and `check_after_body` finds no unparsed bytes and then EOF.
The destination is replaced by `abcdef` and the Host CLI reports success
with `bytes: 6`, despite the declared length of 5.

This violates the new exact-declared-length promise in the help and
`LLM_INSTRUCTIONS.md`, and the root's rejection of contradictory evidence.
The current raw controls supply only Content-Length framing, so none checks
this decoder branch. This finding does not demand rejecting an otherwise
supported chunked response that has no conflicting length.

### R4 — P2: the app route turns a post-body reset into success

Code: `cli-rs/crates/testanyware-agent-client/src/lib.rs:934-941`, with
`cli-rs/crates/testanyware-app/src/streams.rs:274-280`, `:330-342` and
`cli-rs/crates/testanyware-app/src/server.rs:559-561`.

Input: an agent sends a complete Content-Length body; let the app relay
forward that body to the Host CLI, then reset the guest TCP connection
instead of sending FIN. The reset must occur before any orderly guest
half-close. Compare the normal app route with the direct connector.

Derived wrong result: the direct connector exposes the reset to the final
probe as a read error and returns `CONNECTION_DROPPED`, as the producer's
running decision requires. On the app route, `pump` returns the guest read
error, `relay` returns that error, and the server drops its client Unix
socket after closing the guest. It sends no error indicator to the Host
CLI. With the request already consumed and the body already forwarded,
that Unix close presents EOF. The final probe accepts `Ok(0)`, renames the
file and reports `ok: true`. The caller cannot distinguish this guest
transport failure from a clean agent close.

The new framing proof therefore does not reach the mandatory macOS route.
The producer explicitly chose failure for a reset after a complete body
because it cannot prove that no bytes followed. The current app controls
exercise orderly closes and extra/short bodies, not this guest reset
ordering. The existing relay test named for a guest reset drops both
in-memory ends and checks only that the relay finishes; it does not check
what the real Host CLI observes.

### R5 — P2: the old-path runner never reaches its required SIGTERM cases

Code: `scripts/cli-control-old-path.sh:39-64` and
`cli-rs/crates/testanyware-cli/tests/host-cli-control.rs:358-374`,
`:577-624`.

Input: run `task cli:control-old-path` on the stated old revision,
`251bf072e3`. Its exec sends no cancellation, and its interrupted download
leaves its temporary file, as the producer records.

Derived wrong result: each signal test loops over `[SIGINT, SIGTERM]`.
On the old binary, the SIGINT iteration reaches the expected assertion and
panics. Rust then leaves the test function, so the SIGTERM iteration never
runs. The script accepts the assertion's text and prints its old-fail
success. Thus it can exit 0 without executing either old SIGTERM case,
although k222's Done when and the spec's controls explicitly require the
k13 CLI SIGTERM and k7 SIGTERM cases. This does not invalidate the
SIGINT old-fail observation or establish a failure of the new-code run;
it leaves the required old SIGTERM comparison unproved.

## Evidence and limits

- Read the producer diff, its task and commit description, the root brief,
  glossary, lifecycle spec and cited ADRs; inspected the affected protocol,
  client, CLI, schemas, help, error and protocol documentation, control
  script and client/CLI tests. Inspected the client/app connector and relay
  source where the new download check depends on their behaviour.
- The inspected cancel path opens a new connection for each call, also via
  `AppAgentConnector`. Its absolute deadline surrounds the dial, request
  and response body. HTTP 404 is classified before its body is read. The
  terminal cancel reader requires an outcome and uses the existing record
  validator with the cancelled id. No additional defect was established in
  these paths by this review.
- The non-409 exec path still calls `check_status` before reading a body.
  Detached execution bypasses the new listeners and still sends no id;
  dry-run returns before connection setup. The factored typed JSON fields
  preserve the prior mappings. This is a bounded source comparison, not a
  fresh compatibility test result.
- The inspected Linux HTTP/1.0 agent closes its response connection. The
  macOS Hummingbird handler also closes after a request carrying
  `Connection: close`. The extra-byte probe remains under the request
  deadline and can see both Hyper's unread buffer and a later write. No
  additional ordinary-agent close-wait defect was established.
- The source of the locked Hyper 1.9.0, Tokio 1.52.1 and
  signal-hook-registry 1.4.8 dependencies was read from the local Cargo
  registry. These reads support the decoder, signal lifetime and restart
  statements above; no package or tool was installed or updated.
- k222 records that the three old-path cases failed with their known
  defects and passed on the new code. Its task and commit do not attach
  raw output for the full required cli:test, app:routing-test, cli:clippy
  and cli:fmt run set. This review does not infer a pass or a failure for
  that set from the assertions in the task.
- Graph status and coverage could not be queried: `CBM CLI could not start
  because a pre-coordination or unverified CBM generation is active; close
  all CBM sessions and commands, then retry`. No graph generation or
  coverage result is claimed. All findings are bounded to direct source
  inspection and the stated inputs.
- No tests, builds, lint, formatting, agents, VMs or runtime reproductions
  ran. Guest acceptance remains **0/10, unavailable** until the separate
  acceptance trial.
