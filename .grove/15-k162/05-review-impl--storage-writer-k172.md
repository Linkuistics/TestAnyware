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
