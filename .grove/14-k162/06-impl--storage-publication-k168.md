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
- The markers come off the stalled writer, blocking storage call and no
  controller storage calls controls, and those controls pass.
- No control in the node keeps a marker.
- The foundation report section "Evidence storage design (k18)" records the
  delivered behaviour and its limits.
- The full suite passes.
