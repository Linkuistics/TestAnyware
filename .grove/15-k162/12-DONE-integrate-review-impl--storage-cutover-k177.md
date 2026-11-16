# storage-cutover-k177

**Integrates:** storage-cutover-k176

## Goal

Triage the storage cutover review against the agreed contract, apply the
findings that meet the acceptance and material-risk threshold, and leave a
verified cutover for `pipe-capture-k167`.

## Context

Read the findings from the commit for `storage-cutover-k176`, then rederive
them against the current source. The reviewed producer is
`storage-cutover-k166`. Its recorded decisions and
`docs/specs/diagnostic-evidence-storage.md` define this slice's contract.

## Done when

- Every finding has a source-backed classification under the integration
  procedure, with its reasoning recorded here.
- Accepted repairs have focused regression evidence and the repository's
  appropriate required checks. The diagnostic suite passes with the expected
  markers for behavior still owned by the remaining slices.
- The next consumer can build on the cutover without silently inheriting
  unresolved acceptance defects.

## Notes

This sibling repair shares `evidence-writer-k162`'s context and is placed
immediately after its review to preserve the source anchors. Stream routing
remains with `pipe-capture-k167`; the general publication deadline remains
with `storage-publication-k168`.

## Decisions (running log)

- **F1, a dropped control frame leaves storage ready: real issue, repaired.**
  A ready client returns no sequence number only when the outbox cap drops
  the frame. `close()` then waited for a settle that a live writer never
  gave, because storage stayed `ready`. `subjects_after()` stopped forward
  progress without a storage failure. The spec counts an outbox drop as a
  storage failure, so the source was wrong and the spec stays. Both paths
  now fail storage with `outbox_drop`. The writer then reads the end of its
  frame pipe and exits, so the close wait ends.
- **F2, the first failure lost its frame: real issue, repaired.** The
  snapshot read the sequence, operation and record only from an error
  acknowledgement. A missed or late intent gate and an outbox drop carried
  none of them. The spec asks for these fields on the first failure. The
  new `_RunStorage._fail` keeps the frame context only when it sets the
  first failure. A later error acknowledgement cannot replace it, because
  the client keeps a failure acknowledgement only while storage is ready.
  A dropped frame has no sequence number, so the snapshot reports `None`.
  An error acknowledgement still takes precedence when one set the failure.
- **F3, the overflow control could not reach its assertions: real issue,
  repaired in the test.** The call `assert_killed_publication(run,
  RUN_WATCHDOG)` lacked the worker close time, so a run that reached it
  raised `TypeError`. The marker does not accept that type. The control now
  acquires one resource and anchors its teardown on a history entry that
  the flood writes after its stdout. The fault now stops the writer at the
  open of `flood.stdout`, before it drains the stream. The old fault at
  `flood.json` stopped the writer after the stream, so no drop could occur.
- **The overflow marker names both slices.** Until `pipe-capture-k167`, the
  stream goes to a file, and the writer stops only when its close
  transaction pins that file. The close wait then holds the controller.
  After k167, the stream fills the outbox and the drop fails storage, but
  the close wait still holds the controller until `storage-publication-k168`
  bounds it. So k167 keeps the marker and narrows its reason to k168.
- **Regression evidence.** Four `_RunStorage` tests run against the real
  writer: a dropped close frame, a dropped subjects-after frame, a missed
  gate and a late gate. The storage error control and the resumed-writer
  control now also assert the snapshot's first failure. On the controller
  from HEAD, the close test ended by watchdog expiry, and the others failed
  their first-failure assertions. With `--runxfail`, the overflow control
  ends by watchdog expiry in `close()`, which its marker accepts.
- **Verification.** The full suite gives 1066 passed and 6 expected
  failures. The diagnostic storage controls give 10 passed and 6 expected
  failures. Ruff check, ruff format and project pyright pass on the source
  files.
