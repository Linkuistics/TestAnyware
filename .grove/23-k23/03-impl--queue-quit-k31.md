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
