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
