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

## Observed failures on the current build

The controls are in `experiments/exec-reliability/test_storage_controls.py`.
The task `diagnostic:storage-controls` runs them. On 2026-11-17 the run gave 9
expected failures in 54 s. The full suite gave 975 passed and 9 expected
failures. The production code is unchanged.

Each fault fixture wraps the current storage functions of the controller.
These are `write_json`, the stream opens at the start of `_call`, `_hashes`
and `write_identity`. A stall is a read open of a FIFO with no peer. The
watchdog interrupts that open and releases the fault. The `finally` block of
`execute` then disposes of the resources, so no control leaks a marker.

- **Stalled writer.** The fixture stalls every storage call after the
  `acquire` receipt. The watchdog expires at 12 s. Cause: the next storage
  call is the open of the workload stream, and it blocks the controller
  thread. Nothing bounds that call, so teardown and publication cannot start.
- **Blocking storage call.** The fixture stalls the first open of the
  `intent-1` record. The watchdog expires at 12 s. Cause: the controller
  makes that open on its own thread, and no deadline covers it.
- **Storage error.** The fixture removes the write permission from the run
  directory after the `acquire` receipt. The assertion "teardown changed after
  the storage error" fails, and the first resource stays acquired. Cause: the
  stream opens of `cleanup` and `dispose` fail with `EACCES`. `_safe_call`
  records `controller_io_error` and does not start the commands. As root the
  control reports `unavailable`, because root ignores the permission.
- **Writer death.** The fixture raises a broken pipe from every storage call
  after the `acquire` receipt. The assertion "teardown changed after the
  writer died" fails, and the first resource stays acquired. Cause: the same
  as the storage error. A teardown command cannot start without its evidence
  files.
- **Outbox overflow.** The fixture stalls every storage call after the
  `subjects-before` record, and the workload writes 1 MiB. The watchdog
  expires at 12 s. Cause: no outbox exists, so the controller blocks on the
  stream open and never drops output.
- **Readiness failure.** The fixture stalls the open of the plan record. The
  watchdog expires at 5 s. Cause: no readiness bound exists, so the controller
  waits on that open without limit.
- **Intent gate.** The fixture stalls the open of the `intent-2` record. The
  watchdog expires at 12 s. Cause: no intent gate exists, so the controller
  waits on that open without limit.
- **No controller storage calls.** An audit hook records filesystem events
  after the `subjects-before` record. The assertion fails with 71 `open`
  events, the first on `intent-1.json`. Cause: the controller thread opens
  every evidence file itself.
- **Inherited state.** A call reports the kinds of its standard descriptors.
  The assertion fails, because descriptors 1 and 2 are regular files, not
  pipes. Cause: `_call` gives the workload the evidence files as its standard
  output and error.

No control fails from a setup error or from a missing writer parameter. A
forced check showed that the marker fails the test on a `TypeError`, an
`ImportError` and an unexpected pass. A watchdog expiry counts as the expected
failure.

The adapters `storage_outcome` and `writer_descriptors` return `None` on this
build. No control reaches the assertions that use them, because an earlier
behavioural assertion or the watchdog fails first. The slice that removes a
marker binds each adapter to its real outcome.
