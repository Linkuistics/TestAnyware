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
- The markers come off the storage error, writer death, outbox overflow,
  readiness failure and intent gate controls, and those controls pass.
- The full suite passes.

## Notes

A stalled writer can still hold the close wait in this slice.
`storage-publication-k168` bounds that wait.
