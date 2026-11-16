# storage-controls-k163

## Goal

Write the nine required storage controls as tests that fail on the current
build. This slice does not change the production code.

## Context

The controls are in "Test seams and required controls" in
`docs/specs/diagnostic-evidence-storage.md`. The spec says that each control
must fail on the build before the repair. On that build, a control stalls or
fails the existing storage functions through the current test seams.

The writer command parameter does not exist yet. Inject the faults into the
storage paths that the controller uses today. A FIFO with no peer, a removed
write permission and a stopped process are examples. A control that names the
writer, such as writer death, fails today because no writer exists. Record that
reason.

## Done when

- The nine controls are tests in `experiments/exec-reliability/`. They are
  stalled writer, blocking storage call, storage error, writer death, outbox
  overflow, readiness failure, intent gate, no controller storage calls and
  inherited state.
- Each control carries a strict expected-failure marker. The marker names the
  slice that is expected to remove it.
- A watchdog bounds each control, so a stall fails the test and does not hang
  the suite.
- The task `diagnostic:storage-controls` in `Taskfile.yml` runs only these
  controls.
- The task file records, for each control, the observed failure on the current
  build and its cause.
- The storage error control reports `unavailable` when it runs as root.
- The full suite passes.

## Notes

Each control asserts the final behaviour from the spec, not the current
behaviour. The marker then shows the gap, and the strict mode shows its repair.

Expected marker removal by slice:

- `storage-cutover-k166`: storage error, writer death, outbox overflow,
  readiness failure and intent gate.
- `pipe-capture-k167`: inherited state.
- `storage-publication-k168`: stalled writer, blocking storage call and no
  controller storage calls.

The four review controls belong to the slices that build their rules.
