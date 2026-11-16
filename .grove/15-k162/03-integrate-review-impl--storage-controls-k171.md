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
  it; the full suite remains green with the intended strict expected failures.
- The parent brief's existing acceptance remains intact.

## Notes

This sibling is adjacent to the review because it consumes findings anchored
to that review's commit. It precedes `storage-writer-k164` so the remaining
writer work starts from settled controls. The outcome/report adapters remain
deliberate later-slice seams until those slices bind them.
