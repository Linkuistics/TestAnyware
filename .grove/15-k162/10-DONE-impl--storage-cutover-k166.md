# storage-cutover-k166

## Goal

Send every run record from `execute()` through the writer. A storage failure
stops forward progress and leaves teardown unchanged.

## Context

The spec sections are "Phases and bounds" (P2), "Forward stop and failure
reporting" and "Result snapshot". The call streams stay in files until
`pipe-capture-k167`. The publication bounds arrive in
`storage-publication-k168`.

## Done when

- `execute()` writes the plan, the subject digests, the intent records and the
  result through the outbox.
- Each acquisition waits for the acknowledgement of its intent record. The wait
  is at most 2 seconds and runs as scheduler turns.
- A storage failure sets a sticky forward stop. Forward stop prevents later
  acquisitions and workload calls. It does not change cleanup, disposal or
  inspection.
- The result snapshot carries the storage section from the spec. The
  publication field is `pending`.
- `execute()` still returns the run directory as a `Path`.
- `verify()` applies the completion check from `storage-writer-k164` to every
  run directory. The healthy run test in `test_retention.py` passes without
  change.
- The task file records the decision for run directories from before the
  writer.
- The controls switch their fault fixtures to the writer wrapper. Their
  assertions do not change.
- The markers come off the storage error, writer death and readiness failure
  controls, and those controls pass.
- Tests in this slice show the intent gate with a writer that stalls on
  intent-2 and resumes after the gate deadline. The second resource is never
  acquired, forward progress stops and the first teardown does not change.
- Each control that keeps its marker fails under its watchdog. It leaves no
  live writer and no stopped fixture process.
- The full suite passes.

## Notes

A stalled writer can still hold the close wait in this slice.
`storage-publication-k168` bounds that wait. The intent gate and outbox
overflow controls keep a writer stalled through close, so their markers stay
until that slice. The resumed stall above does not establish the persistent
stall case.

## Decisions (running log)

- Run directories from before the writer fail `verify()`. They have no
  storage-close record and no storage section in the result. So they cannot
  show that publication completed. `verify()` applies the completion check to
  every run directory and gives no exemption for them. The report still shows
  their identity, file and stream states, and the completion reasons name the
  missing storage-close record. `test_retention.py` holds a test for this case.
- The storage error control injects `EACCES` in the writer with a new
  `deny:NAME` fault in `writer_faults.py`. The old fixture removed write
  permission from the run directory. In this slice the controller still opens
  the call stream files in that directory. A read-only directory would
  therefore also stop the teardown calls, and the control requires that
  teardown does not change.
- An error acknowledgement after readiness fails storage. The failure cause is
  the errno name, and the client keeps the acknowledgement as `failure_ack`.
  The result snapshot reads the sequence number, operation and record name
  from it.
- End of file on the acknowledgement pipe now fails storage as `writer_exit`.
  Only the writer holds the write end, and it closes that end only when it
  exits. The cause `ack_eof` from `storage-client-k165` no longer occurs. The
  client tests accept both names.
- A resource whose intent gate fails is never acquired. It therefore does not
  join the teardown list. The resources that the controller acquired before the gate
  keep their teardown.
- The wait for the subjects-after acknowledgement is at most 5 seconds. This
  is the largest W/2 in the spec. `storage-publication-k168` replaces it with
  the bound S + W/2.
- `execute()` returns `RunDirectory`, a `Path` subclass. Its `storage`
  attribute holds the storage outcome. Callers that use the value as a `Path`
  do not change. Python 3.12 supports a direct subclass of `Path`:
  https://docs.python.org/3.12/whatsnew/3.12.html#pathlib
- The close wait has no bound in this slice. `storage-publication-k168` adds
  the bound. A writer that stays stopped through close therefore holds the
  run until the control watchdog kills it.
- The live-writer check in the controls uses `pytest.fail`, not `assert`. An
  `xfail` marker accepts an `AssertionError` from the teardown phase too. So
  an `assert` would hide a writer that a marked control left alive. A mutant
  that removed the writer cleanup showed this.
- The outbox overflow control stalls the writer at `flood.json`. The flood
  stream stays a file until `pipe-capture-k167`, so the stream record name
  `flood.stdout` cannot carry the stall yet.
