# storage-writer-k164

## Goal

Build the writer process and the completion check in `retention.py`. The
controller does not use the writer in this slice.

## Context

The spec sections are "Ownership", "Channel contract", "Phases and bounds"
(the close transaction only), "Completion authority" and "Controller-crash
limits". The review leaf `diagnostic-disposal-k160` placed the change to
`retention.py` in the same slice as the writer. The close record and the check
that accepts it form one working increment.

Today `execute()` writes no close record, and the healthy run test in
`test_retention.py` requires `verify()` to accept its run directory. So
`verify()` keeps its current rules in this slice. `storage-cutover-k166`
switches `verify()` to the completion check when `execute()` starts to produce
the close record.

## Done when

- A writer module applies frames to files relative to its run-directory
  descriptor. It uses exclusive creation and does not follow links.
- The writer refuses a reserved record name in a wrong operation, a path
  separator in a name and a payload over 1 MiB. Each refusal is acknowledged
  with its errno and operation.
- The writer acknowledges each frame with its sequence number. A storage error
  acknowledgement carries the real errno.
- On a close frame, the writer writes `storage-close`, then the identity file
  over every file including `storage-close`. It acknowledges the lengths and
  SHA-256 digests, then exits.
- On end of input without a close frame, the writer applies the complete
  frames, discards a partial frame and writes `eof_without_close`. It writes no
  identity file.
- A completion check in `retention.py` applies the rules in "Completion
  authority" to a run directory. A missing or changed `storage-close` makes the
  run incomplete. So does `eof_without_close`. Tests apply the check to
  directories that the writer produces.
- `verify()` does not call the completion check yet.
- The close record review control passes. It also fails on a writer build that
  writes the identity file before `storage-close`.
- Tests drive the writer with a test client over pipes.
- The full suite passes, and `execute()` does not change.

## Notes

The writer holds only its three standard descriptors and its run-directory
descriptor. The inherited state control checks this after the cutover.

The existing callers and tests read files such as `result.json`,
`plan.json` and `result-identity.json`. Map each record name to the file name
that they read today. If the spec's record names cannot keep those file names,
record the conflict before the cutover depends on it.
