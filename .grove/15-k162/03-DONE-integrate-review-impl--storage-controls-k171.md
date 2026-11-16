# storage-controls-k171

**Integrates:** storage-controls-k170

## Goal

Triage and integrate the findings of `storage-controls-k170` before the writer
slices rely on the nine storage controls as acceptance gates.

## Context

Read the findings from that review's commit and inspect the producer artifact
`storage-controls-k163`. The artifact and its contract are
`experiments/exec-reliability/test_storage_controls.py` and the nine required
controls in `docs/specs/diagnostic-evidence-storage.md`. The task
`diagnostic:storage-controls` runs this suite.

## Done when

- Each finding is verified against the source and classified using the
  integration procedure's acceptance and material-risk threshold.
- Actionable findings are repaired and verified through the project's tasks.
- The producer's recorded failure evidence is updated where a repair changes
  it. The full suite remains green with the intended strict expected failures.
- The parent brief's existing acceptance remains intact.

## Notes

This sibling is adjacent to the review because it consumes findings anchored
to that review's commit. It precedes `storage-writer-k164` so the remaining
writer work starts from settled controls. The outcome/report adapters remain
deliberate later-slice seams until those slices bind them.

## Decisions (running log)

- **F1, final absence is not a forward stop: real issue, repaired.** The
  source shows the defect. Disposal removes the marker, so an
  acquisition followed by disposal passed the old check. Each resource
  command and the workload now append one entry to a history file after
  their action succeeds. The storage-error, writer-death and intent-gate
  controls assert that the history holds no acquisition of the second
  resource.
- **F2, teardown is only partly observed: real issue, repaired.** The source
  shows the defect. The cleanup fixture wrote bytes and changed no
  state, and a returned `close()` does not prove a reap. The helper
  `assert_teardown_unchanged` now asserts cleanup, disposal and inspection
  in order, each within its budget and slack. It also asserts one worker
  start, and a worker shutdown with the outcome `reaped` and no overrun.
- **Blocking storage call: contract read, no teardown assertion.** On the
  final build the `intent-1` record misses its gate, so nothing is
  acquired. The spec says that the result matches the stalled-writer
  control. The control asserts no acquisition, no workload, a reaped
  worker and a killed publication. It does not assert the teardown of a
  resource that the run never acquired.
- **Stalled writer: keep the stall after the `acquire` receipt.** This
  point is after readiness, as the spec requires. It also gives an owned
  resource, so the teardown assertions have an effect.
- **F3, whole-run watchdogs do not check phase bounds: real issue,
  repaired.** `StorageOutcome` now carries phase receipts, the KILL time
  and the reap time. The publication controls assert the window, KILL at
  P and the reap by P plus 2 s. The readiness control asserts its bound and
  the reap within 2 s of its deadline. The intent controls assert the
  half-second gates. The watchdogs
  stay as outer bounds. `TOLERANCE` is 0.5 s of scheduling lateness,
  because the spec makes the timing conditional on scheduling.
- **Adapter fields are a later-slice contract.** The new `StorageOutcome`
  fields are the shape that `storage-cutover-k166` and
  `storage-publication-k168` bind. The adapters still return `None` on this
  build, and no control reaches them.
- **Pyright noise: accepted.** Pyright reports 113 errors in the file,
  against 96 in the producer's file. The extra errors come from more uses
  of fixture parameters without annotations. No project task runs pyright
  on test files.
- **The unused `peer()` helper is removed.** No control calls it after the
  change to the recorded resource commands. `PEER` stays as a hashed
  subject.

## Verification (2026-11-17)

- `mise exec -- task diagnostic:storage-controls`: 9 expected failures in
  54 s. Each control still fails for its intended reason. Five controls
  fail on the watchdog. The storage-error and writer-death controls fail
  with "teardown actions were []". The storage-call control fails with 66
  audit events, and the inherited-state control fails on regular files.
- `mise exec -- task diagnostic:test`: 975 passed and 9 expected failures.
- `mise exec -- task diagnostic:format`: 16 files unchanged. Ruff check
  passes.
- A scratch positive control ran on the current build with no fault. A
  healthy run passes `acquired`, `workload_finished` and
  `assert_teardown_unchanged`. In a healthy two-resource run, the old
  marker check passes for the second resource, and the history still finds
  its acquisition. A cleanup command that does nothing passes the old
  marker check and fails `assert_teardown_unchanged`.
- The producer's evidence in `01-DONE-impl--storage-controls-k163.md` now
  records the second run, the history file, the new assertion text and the
  66 audit events. It also replaces the claim that no control leaks a
  marker.
