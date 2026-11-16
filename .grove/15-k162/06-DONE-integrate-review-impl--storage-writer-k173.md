# storage-writer-k173

**Integrates:** storage-writer-k172

## Goal

Triage the committed review of the evidence writer and settle the actionable
findings before the controller client depends on its protocol.

## Context

Read the findings from the commit for `storage-writer-k172`, which reviews
producer `storage-writer-k164`. The contract is
`docs/specs/diagnostic-evidence-storage.md`. Apply the integration kind's
acceptance and material-risk threshold to each finding against the source.

## Done when

- Each finding has a recorded classification and supporting evidence.
- Any repair needed for the existing writer acceptance is implemented and
  verified, including a control that fails without the repaired rule.
- The diagnostic checks pass through the repository's Taskfile entry points,
  and the client slice has a settled frame and completion contract to consume.
- The integration records any accepted trade-off or remaining limit without
  expanding the agreed storage scope.

## Notes

This sibling is placed immediately after the review, before
`storage-client-k165`, so no intervening implementation moves the source
coordinates or changes the reviewed protocol. The findings remain in the
review's commit rather than becoming this task's charter.

## Decisions (running log)

- **F1, an unpinnable file still gives an `ok` close: real issue,
  repaired.** A reproduction showed the defect. A run file with mode
  000 gave an `ok` close ack, exit 0 and an identity record. `_pin` now
  raises the real errno and call for `open`, `fstat`, `read` and `close`.
  The close ack then reports `failed`, the writer writes no identity
  record, and it exits 1. Subject digests keep their `unavailable` text,
  because the spec supports that case.
- **A non-regular run file fails with `EFTYPE` and the call `fstat`.** No
  call fails for a FIFO or a socket, so no real errno exists. `EFTYPE`
  names a wrong file type on macOS. Where it is absent, the writer uses
  `EINVAL`. The failure keeps the spec rule that an incomplete listing
  gives no identity record.
- **F2, append writes through a hard link: real issue, repaired.** A
  reproduction showed the defect. The append wrote to an outside file
  and to `plan.json` through hard links, with ack `ok`. The writer now
  records the device and inode of each create. After the append open, it
  refuses a different inode with `ENOENT`. It refuses a second link with
  `EMLINK`. Both refusals use the call `validate` and count as refused.
- **F3, a failed `close(2)` escapes the ack path: real issue, repaired.**
  The source showed the defect, and the fault wrapper reproduced it. A
  failed close now gives a `failed` ack with the call `close`. When a
  write and its cleanup both fail, the ack keeps the write error. The same
  rule covers the pin, listing and setup descriptors. The writer does not
  retry a close, because macOS and Linux release the descriptor even when
  close fails.
- **Fault controls use a wrapper, not a mock.** `writer_faults.py` runs
  the real writer with EIO on chosen calls for chosen run files. It
  replaces only the `os` name in the writer module. The real close runs
  first, so an injected fault leaks no descriptor. Pyright type-checks the
  wrapper in strict mode.
- **Each control failed before its rule.** All ten new controls failed on
  the producer's writer. Four mutants of the repaired writer each failed
  their own control. The mutants removed the inode test, the link-count
  test, the primary-error rule and the read-error rule.
- **Contract for `storage-client-k165`.** The frame protocol has no new
  field. An append can now give a `refused` ack with `ENOENT` or
  `EMLINK`. A close can give a `failed` ack with the call `open`, `fstat`,
  `read`, `close` or `write`. A `failed` close ack means no completion, as
  the spec already says. The spec now states the append rule.
- **Limit: a failed identity write can leave a partial identity file.**
  Step 3 is the failed step, so the writer writes nothing after it. The
  close ack is `failed`, so completion still refuses the run. Removing the
  partial file is outside the agreed storage scope.
- **Verification.** `mise exec -- task diagnostic:test` gives 1027 passed
  and 9 expected failures. The nine storage controls stay strict expected
  failures. Ruff check, ruff format and project pyright pass.
