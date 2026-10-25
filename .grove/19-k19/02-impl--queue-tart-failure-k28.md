# queue-tart-failure-k28

## Goal

A tart that does not answer, or answers slowly, costs the app no slot and no
deadline. The app keeps its last answer and says that tart is silent. It frees a
slot only on a later read of tart.

## Context

- The brief of `queue-external-k19` holds the full "Done when". This child takes
  the part that `queue-observer-k27` leaves.
- Spec `docs/specs/admission-queue.md`. Read "The count" from "When tart does not
  answer" to its end, and "The wait and its end". Read the command surface for the
  remedy and the lines on stderr. Read the slow-tart cases of "Test seams".
- `queue-observer-k27` built the observer, the numbering and the count. Read its
  decision log first.

## Done when

- An observation also fails when one call does not answer within 5 s. The bound
  applies to each call. A call that answers after its observation failed changes
  nothing. The time of an observation is the moment when its last call returns.
- A stop of the app removes its VM from the answer only on a later `tart list`
  that does not name it. That list must start after the last call of the
  observation returned. A `tart list` that fails proves nothing.
- The read of a stop is not an observation. It changes neither `observed_at` nor
  `unanswered_since`, and it is not the first answer of a run.
- The exit of a `tart run` removes no VM from the answer. It ends only the hold of
  the admission that started that `tart run`.
- A newer answer replaces the last answer whole, with each removal that a stop
  made.
- A waiting start writes a line that says that tart does not answer, with the
  time of the last answer when one exists. The remedy of `VM_LIMIT_REACHED` says
  so too.
- `unanswered_since` stays in memory.
- The tests of the count cover a stop that began after the last answer, with
  failed observations after it. They cover a newer answer after such a stop that
  still reports the VM running. They also cover the exit of a `tart run` that the
  app learns after an answer. That answer shows a replacement under the same name.
- The protocol tests use a fake `tart` with a slow `tart get`, a `tart list` that
  does not answer, and a fake guest that starts outside the app. They cover a
  wait that runs out during a slow observation, and a slot that frees only after
  the deadline. They also cover a start without `--wait` that arrives during such
  an observation.

## Notes

- The pure functions of the count and of the details of `VM_LIMIT_REACHED` stay
  the home of each rule, as in `queue-observer-k27`.
