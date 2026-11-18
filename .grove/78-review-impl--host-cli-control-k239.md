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
