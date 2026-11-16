# storage-controls-k163

## Goal

Write the nine required storage controls as tests that fail on the current
build. This slice does not change the production code.

## Context

The controls are in "Test seams and required controls" in
`docs/specs/diagnostic-evidence-storage.md`. The spec says that each control
must fail on the build before the repair. On that build, a control stalls or
fails the existing storage functions through the current test seams.

The writer command parameter does not exist yet. Inject each fault through one
fault fixture per control. On this build, the fixture stalls or fails the
storage paths that the controller uses today. A FIFO with no peer, a removed
write permission and a stopped process are examples. For writer death, fail a
P2 storage call with a broken pipe. The slice that removes a marker switches the
fixture to the writer wrapper and keeps the assertions unchanged.

A missing writer or a missing writer command parameter is not a recorded
failure. Each recorded failure must come from a behavioural assertion of the
spec after the fault is injected. A teardown past its ceiling or a forward
acquisition after the fault are examples. If a control reaches no behavioural
failure on this build, record that result and report it to the review. Do not
substitute a setup failure.

## Done when

- The nine controls are tests in `experiments/exec-reliability/`. They are
  stalled writer, blocking storage call, storage error, writer death, outbox
  overflow, readiness failure, intent gate, no controller storage calls and
  inherited state.
- Each control carries a strict expected-failure marker. The marker names the
  slice that is expected to remove it. The marker accepts only an assertion
  failure or a watchdog expiry. An import error, a type error or another setup
  error fails the test.
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

- `storage-cutover-k166`: storage error, writer death and readiness failure.
- `pipe-capture-k167`: inherited state.
- `storage-publication-k168`: stalled writer, blocking storage call, outbox
  overflow, intent gate and no controller storage calls. Each of these controls
  keeps a writer stalled, so it needs the bounded publication. Outbox overflow
  also needs the call output in the outbox from `pipe-capture-k167`.

The four review controls belong to the slices that build their rules.
