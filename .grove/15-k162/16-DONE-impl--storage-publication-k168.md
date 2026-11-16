# storage-publication-k168

## Goal

Bound publication and stop the writer, so that a stalled writer cannot hold
the controller. Return the terminal outcome to the caller.

## Context

The spec sections are "Phases and bounds" (P3), "Terminal outcome",
"Timing conditions" and "Test seams and required controls".

## Done when

- Publication starts at S and ends at P. P is S plus W, and W is at most
  10 seconds.
- The subject digests after the run are taken by S plus W/2. If they are late,
  their state is `unavailable:storage`.
- The controller sends one close frame and drains acknowledgements until P.
- At P, the controller sends KILL to the writer. It reaps the writer by P plus
  2 seconds, or it records a residual writer. A residual never blocks the
  controller.
- A new entry point returns the terminal outcome from the spec. Its
  publication field is `complete`, `incomplete`, `killed` or `residual`. A
  passing verdict requires `complete`.
- `execute()` still returns the run directory as a `Path`.
- Phases P2 and P3 import no module. Error records carry the type and the
  message without a traceback.
- The audit hook control finds no file system event and no import event from
  readiness to the end of publication.
- The result snapshot review control passes. A writer killed after the result
  frame and before the close frame leaves the outcome `incomplete`.
- The markers come off the stalled writer, blocking storage call, outbox
  overflow, intent gate and no controller storage calls controls, and those
  controls pass.
- No control in the node keeps a marker.
- The foundation report section "Evidence storage design (k18)" records the
  delivered behaviour and its limits.
- The full suite passes.

## Decisions (running log)

- The plan field `publication_window` declares W. Validation refuses a value
  that is not finite, not positive or above 10 seconds. The controls declare
  1 second through `declare()`.
- `begin_publication()` sets S once, after `inspection_state.close()`.
  `subjects_after()` and `close()` call it too, so a direct call in a repair
  test still has a phase.
- An acknowledgement carries no receipt time. A subjects-after
  acknowledgement therefore counts only when the controller reads it by
  S plus W/2, as at the intent gate.
- The client gains `kill(reap_deadline)` and `expect_exit(reap_deadline)`.
  Publication passes P plus 2 seconds to both, so a late turn cannot move
  the reap deadline. A writer that acknowledged close gets no KILL.
- `killed` counts only a KILL from the controller, at P or after an
  exception. The client's own KILL after a readiness failure leaves the
  outcome `incomplete`.
- `execute_outcome()` is the new entry point, and it returns `RunOutcome`.
  `execute()` returns `execute_outcome(...).run_dir`, so callers keep a
  `Path` with the `storage` attribute.
- The only controller file call after readiness was the `/dev/null` open of
  `subprocess.DEVNULL`. The controller now opens the null device in P0 and
  passes its descriptor as command input.
- Each controller error record adds `error_type` beside its message. The
  messages stay unchanged, because existing tests compare them exactly. No
  record held a formatted traceback before this slice.
- The result snapshot review control kills the writer from the test, after
  the result acknowledgement and before the close frame. Its negative build
  pins the result before the snapshot and fails on the digest assertion.
- Five markers come off here, and the `removed_by` helper goes with them.
  The final full suite passed with 1083 tests and no expected failure.
  Ruff and pyright passed.
