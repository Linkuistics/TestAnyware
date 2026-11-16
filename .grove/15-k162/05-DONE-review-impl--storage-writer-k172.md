# storage-writer-k172

**Reviews:** storage-writer-k164

## Goal

Review the evidence writer adversarially before the client and cutover slices
build on its frame protocol and its completion check.

## Context

Read the producer commit for `storage-writer-k164`. The writer is
`experiments/exec-reliability/writer.py`, and the test client is
`experiments/exec-reliability/writer_client.py`. The completion check is
`completion()` in `experiments/exec-reliability/retention.py`. The tests are
`experiments/exec-reliability/test_writer.py`, and the close record control is
at the end of `experiments/exec-reliability/test_storage_controls.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its sections "Phases
and bounds", "Channel contract", "Completion authority" and "Test seams and
required controls" give the contract. The producer leaf records each decision about
the wire format, the errno values and the record file names.

Run the checks with `mise exec -- task diagnostic:test`. `execute()` does not
use the writer yet, so the nine required controls keep their markers.

## Done when

- Check that no frame can make the writer write outside the run directory.
  Try names, links, renames and case variants of the reserved names.
- Check that each refusal and each storage error acknowledgement carries the
  real errno and the operation.
- Check that a frame split at any byte boundary before end of input gives the
  same files as one write. A partial trailing frame must leave no file.
- Check that the close transaction writes storage-close before the identity
  record. The identity record must list storage-close with the bytes on disk.
- Check that `completion()` refuses each directory that the spec calls
  incomplete. A directory that it accepts for the wrong reason is a finding.
- Check that the writer holds only its three standard descriptors and the
  run-directory descriptor while it applies frames.
- Check that the wire format and the result storage section are enough for
  the client and cutover slices. Name any field that they must add.

## Notes

The close record control has a second test. It runs a wrapper that writes the
identity record before storage-close and expects the control to fail. The
review can use the same wrapper shape for other mutant builds.

## Decisions (running log)

- This is an inspection-only review under `grove-review-impl`. The instruction
  above to run `diagnostic:test` does not apply to this kind. No tests, builds,
  linters or formatters ran, and no production or test files changed.
- Reviewed producer commit `a9f1d815d6f035e096bf6af313b220abd549312e`
  (`storage-writer-k164`) against the current source and the storage spec.
  Graph discovery returned no writer symbols. Coverage reported no recorded
  gaps but freshness was `not_tracked`, with generation
  `2026-11-15T02:19:52Z`. Read the complete writer, test client, retention
  module and writer tests directly, plus the close-record control and config
  diff. Findings below are derived from source; their scenarios were not run.
- The findings affect the existing containment and error-reporting acceptance
  criteria. An integration sibling must triage them before `storage-client-k165`
  consumes this protocol. This is a repair handoff within `evidence-writer-k162`,
  with no new subsystem or broader sandbox requirement.

## Findings

### 1. [P1] Identity read failures still produce a successful close

`experiments/exec-reliability/writer.py:343-364` converts failures of the
identity file's `open`, `fstat` or `read` into an entry with an `error` string.
`experiments/exec-reliability/writer.py:386` includes that entry in the listing,
and `experiments/exec-reliability/writer.py:416-449` writes the identity and
acknowledges close with `status: ok`. The original errno is lost and the
writer's failed-frame count remains unchanged.

For example, make a file read raise `OSError(EIO)` while close computes its
pin. The source then takes the successful close path and `run()` returns zero.
`completion()` subsequently rejects the same entry as `unpinned`. Publication
can therefore report `complete` to the controller while the retained directory
is incomplete: the terminal rule uses an ok close acknowledgement and a reap,
without reading evidence back.

This violates `docs/specs/diagnostic-evidence-storage.md:125-127` and its
terminal publication rule at lines 259-268. Required repair: an identity
storage failure must fail close with its real errno and syscall, create no
identity after that failure, and exit. Subject-hash unavailability is a
different, explicitly supported case. The existing healthy close and
identity-first mutant controls do not cover an identity read failure. The
integration should retain a fault control that checks the close
acknowledgement, exit status and absence of identity.

### 2. [P2] Append can modify an outside file through a replacement hard link

`experiments/exec-reliability/writer.py:261-263` authorises append using only
the name in `self.created`. `experiments/exec-reliability/writer.py:321-341`
then reopens that name with `_APPEND`. `O_NOFOLLOW` prevents a symbolic-link
replacement, but it does not refuse a hard link or check that the inode is
the file this writer created.

Concrete sequence on one volume: create `stream` through the frame protocol;
unlink that directory entry; hard-link an outside regular file at `stream`;
send an append frame for `stream`. The name still passes `self.created` and
`openat` resolves the hard link to the outside inode, so the writer appends to
the outside file and acknowledges success. A hard link from an existing
reserved record to this name can likewise bypass the reserved-name protection.

This fails this review's explicit names/links/renames containment criterion
and the producer's statement that append extends a record the writer created.
`experiments/exec-reliability/test_writer.py:164-173` exercises precisely this
replacement shape with a symbolic link; it does not exercise a hard link.
Required repair: reject replacement or linked append targets so that a
previously admitted name alone cannot authorise a write to another file.
Retain the outside-file and reserved-file bytes as controls. This finding
does not claim protection from arbitrary hostile concurrent code; it concerns
the same deterministic replacement scenario already in the accepted tests.

### 3. [P2] Descriptor-close errors bypass the frame error acknowledgement

`experiments/exec-reliability/writer.py:334` calls `os.close` in a `finally`
block outside the `OSError` conversion. A close that reports a delayed storage
error such as `EIO` escapes as a raw exception. `handle()` and `close()` catch
only `Rejected`, so this frame receives no acknowledgement carrying its
sequence, operation, errno and `call: close`. A close error can also replace
an already converted write error during exception unwinding. The identity
read helper has the same unconverted close at
`experiments/exec-reliability/writer.py:364`.

This violates the per-frame storage-error contract at
`docs/specs/diagnostic-evidence-storage.md:167-169`, and the close-transaction
failure rule when it occurs during publication. A controller eventually
observing EOF does not recover the actual errno. Required repair: route these
close errors through the acknowledgement path while retaining the primary
storage failure if cleanup also fails. Add a fault control for close failure;
the current real-errno test at
`experiments/exec-reliability/test_writer.py:155-161` checks only `open/EEXIST`.

## Other inspected acceptance evidence and limits

- Name validation excludes empty names, `.`, `..`, slash and NUL. Reserved
  record names and their file aliases are compared with case folding.
  Exclusive create and `O_NOFOLLOW` cover the planted symbolic-link tests;
  the run-directory descriptor anchors creation across a directory rename.
  Finding 2 limits the append containment conclusion.
- For in-limit frames, `FrameReader.frames()` waits for both lengths, the
  complete header and the complete payload before consuming the frame. Keeping
  that buffer across reads makes the ordinary-frame result independent of
  input splits. EOF dispatches no partial trailing frame and creates no
  identity. The tests cover a partial payload and EOF during an oversized
  skipped payload; they do not enumerate every split boundary.
- The normal close path creates storage-close before identity, lists the
  close record and returns byte pins for both. The committed control checks
  healthy, missing and changed close records, and its identity-first wrapper
  is expected to fail for the missing listing entry. Finding 1 limits the
  conclusion for unsuccessful identity observations.
- `completion()` checks the identity listing, changed or unpinned files,
  unlisted entries, orderly close, zero refused/failed counts, sequence
  coverage and each supplied result digest. The new tests assert rejection
  of missing/changed close, missing identity, an extra file, changed bytes
  and a result sequence beyond last-applied. `verify()` intentionally keeps
  its prior rules until `storage-cutover-k166`.
- The writer retains its standard descriptors and run-directory descriptor
  between operations. Individual file and listing descriptors have local
  cleanup, subject to finding 3. The recorded descriptor test observes the
  settled process; it does not prove spawn-side inheritance, which belongs
  to the client and cutover slices.
- Setup supplies the resolved run path, subject values and subjects-before
  pin. Frame acknowledgements carry seq/op/status and, on converted errors,
  errno/call. Subjects-after writes no file; close supplies both final pins.
  `result.storage.sequence` and the per-record `digests` mapping suffice for
  this slice's completion check. The client/cutover must still supply the
  full snapshot fields required by the spec; this review found no additional
  protocol field needed for those slices beyond correcting the error paths.
- The producer log records the intended close mutant and case-folding mutant
  results. It retains no full-suite command result or test-count transcript.
  This review therefore makes no fresh or independently verified suite-pass
  claim.
