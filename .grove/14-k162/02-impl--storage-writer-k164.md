# storage-writer-k164

## Goal

Build the writer process and make `retention.py` accept a run only through the
writer's close record. The controller does not use the writer in this slice.

## Context

The spec sections are "Ownership", "Channel contract", "Phases and bounds"
(the close transaction only), "Completion authority" and "Controller-crash
limits". The review leaf `diagnostic-disposal-k160` placed the change to
`retention.py` in the same slice as the writer. The close record and the only
reader that accepts it form one working increment.

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
- `retention.py` verifies a run by the rules in "Completion authority". A
  missing or changed `storage-close` makes the run incomplete. So does
  `eof_without_close`.
- The close record review control passes. It also fails on a writer build that
  writes the identity file before `storage-close`.
- Tests drive the writer with a test client over pipes.
- The task file records the decision for run directories from before the
  writer.
- The full suite passes, and `execute()` does not change.

## Notes

The writer holds only its three standard descriptors and its run-directory
descriptor. The inherited state control checks this after the cutover.
