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

## Decisions (running log)

- Wire format. A frame is two big-endian 32-bit lengths, a JSON header and the
  payload. The header holds `seq`, `op`, `name` and the fields of its
  operation. The fixed lengths let the writer find a partial trailing frame at
  end of input. Each acknowledgement is one JSON line on standard output.
- Operations. `setup` creates the run directory and writes `plan` and
  `subjects-before`. `create` makes a new record. `append` adds bytes to a
  record that this writer created. `subjects` returns digests and writes no
  file. `result` creates the result record. `close` runs the close
  transaction.
- Reserved names. A `create` or `append` frame cannot name `plan`,
  `subjects-before`, `storage-close` or `result-identity`. Only a `result`
  frame creates `result`, and `append` can then extend it. A name is reserved
  when the record name or its file name matches.
- File names. A reserved record keeps the file name that callers read today:
  `plan.json`, `result.json`, `subjects-before.json`, `storage-close.json` and
  `result-identity.json`. Every other record name is its file name. So the
  spec's record names cause no conflict with the existing callers.
- The setup frame carries the evidence root as an absolute path. A setup
  failure is then a readiness acknowledgement with a real errno. The writer
  resolves the root and reports the run directory path in the readiness
  acknowledgement, so the controller never resolves the root.
- Errno values for refusals: `EINVAL` for a bad name, `EPERM` for a reserved
  name in a wrong operation, `EMSGSIZE` for an oversized payload or header,
  `EPROTO` for a malformed header or a sequence number that does not increase,
  `EBADF` for a frame before setup, and `EALREADY` for a second setup.
- The writer is self-contained and imports only the standard library. In
  isolated mode, the script directory is not on the module path, so it cannot
  import `retention.py`. Its identity record uses the same format as
  `retention.write_identity`.
- The result storage section that the completion check reads is
  `result["storage"]` with `sequence` and `digests`. `digests` maps each
  record name to `length` and `sha256`. `storage-cutover-k166` serialises this
  shape.
- The close record control drives the writer with the test client. The
  mutant build is a wrapper that runs the real writer with the identity step
  before the storage-close step. `storage-cutover-k166` can switch the healthy
  run to `execute()` when `execute()` uses the writer.
- The test client in `writer_client.py` is blocking and serves only tests.
  The controller client, with nonblocking pipes and a bounded outbox, belongs
  to `storage-client-k165`.
- The close record control is two tests. `test_control_close_record` runs the
  real writer and passes. The second test runs the identity-first wrapper and
  expects the control to fail with the reason "the identity listing lacks the
  storage-close record".
- No required control passes in this slice, so all nine expected-failure
  markers stay. Each one needs `execute()` to use the writer, and `execute()`
  stays unchanged until `storage-cutover-k166`.
- The descriptor test checks four descriptors: 0, 1, 2 and one directory.
  Setup closes the evidence-root descriptor after it opens the run directory,
  so the run-directory descriptor can have the number 4.
- `writer.py` and `writer_client.py` join the strict pyright list. The writer
  is production code, and `retention.py` stays outside the list as before.
- The reserved-name check compares case-folded names. A default macOS volume
  ignores case, so a `create` frame for `RESULT.json` would otherwise open
  `result.json`. Exclusive create still refuses an existing file, but a
  `create` frame before the result frame could then take the result name.
  Three test cases fail on a check without case folding.
