# queue-quit-k31

## Goal

A quit of the app loses no slot and leaves no half-made VM. The app stops
admitting and cancels its starts. It waits up to 10 s for their cleanup. It keeps
the record of a start that the 10 s leave unfinished.

## Context

- Spec `docs/specs/admission-queue.md`, "A quit, a crash and a restart", the
  paragraph "A quit", and the scenario "A quit while starts wait".
- The brief of `queue-start-record-k23` holds the full "Done when".
- `queue-record-decision-k29` wrote the start record. This child relies on it for a
  start that the 10 s leave unfinished.
- `applicationWillTerminate:` in `testanyware-app/src/shell.rs` and the signal
  handlers of `main.rs` are the two ways to a quit. A quit by Homebrew for an
  upgrade is a SIGTERM.

## Done when

- A quit of the app stops admitting. It cancels each waiting start and each
  admitted start that has no result yet, as a closed connection does. It waits up
  to 10 s for their cleanup, and then it exits.
- Each client of such a start gets `APP_UNREACHABLE` with cause `quitting`. A
  waiting golden creation gets the same error. Each VM that started keeps running.
- A start that the 10 s leave unfinished keeps its start record, and the next start
  of the app decides for it.
- A protocol test quits the app while starts wait and one start boots. Each client
  fails with `APP_UNREACHABLE` and cause `quitting`, no clone or VM of those starts
  remains, and each VM that started keeps running.
- A test with a cleanup that takes longer than 10 s shows that the app exits at the
  limit and the record stays.

## Notes

- The tests may shorten the 10 s with a test-mode argument, as other bounds do.

## Decisions (running log)

**One `Quit` object per run of the app counts the starts and holds, and cancels them.**
`quit.rs` holds a parent `CancellationToken`, a count of registered threads and a `Condvar`.
`enter` gives a start or a hold a guard and a child token, and it returns `None` once the quit has
begun, so the app admits nothing after that. The child token takes the place of the token that the
response body cancels, so a quit and a closed connection cancel a start by the same path. The queue
needed no change: a cancelled waiter or admitted start already leaves with no slot lost.

**The guard lives as long as the thread, so the wait covers the cleanup.** The start thread and the
hold thread take the guard first. It drops when the thread ends, after the cleanup of the clone or
the release of the slot. A start that the limit leaves unfinished never reaches the removal of its
record, so the record stays with no code of its own. The app exits at the limit by
`runtime.shutdown_background()`, because a blocking task would otherwise delay the exit.

**The client learns `quitting` from a stream with no final event.** The spec already states that a
stream which ends with neither event is a quit, so the app sends no new event. A request that
arrives after the quit began gets such a stream at once (`quitting_response`).

**A result that is sent before the cancel keeps its VM.** The thread of a start stops a VM that
nobody learns of. A quit that begins after the send is no client that left, so the check reads
`quit.is_quitting()` and the VM keeps running.

**Three ways to a quit, one function.** `finish_and_say` runs for the SIGTERM and Ctrl-C of test
mode, for `applicationWillTerminate:` and for a new signal task of the menu bar app. That app had no
SIGTERM handler, so Homebrew's SIGTERM killed it with no cleanup. The limit counts from the first
call, so a second call does not wait again.

**`--test-quit-wait-ms` shortens the 10 s, and the fake `tart` gets `delete-delay`.** The slow
`delete` is a child of the app, so it goes on after the app exits. The test then starts the app
again and sees the decision for the record.

**The tests discriminate.** The test with waiting starts failed when the call to `finish_and_say`
was replaced by a no-op (the clones stayed). `quit.rs` has 6 unit tests, and `app-vm-queue.rs` has 2
new protocol tests with a toggle agent that answers `/health` only while the test allows it.
`task app:test` and `task app:routing-test` pass.

**The node closes.** `queue-start-record-k23` compared with its three children: the record and its
removal, the decision before the socket, the retry, the status results, the quit with its 10 s, and
the three kinds of protocol test are each done. No gap, and nothing to promote upward.
