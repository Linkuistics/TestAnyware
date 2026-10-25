# queue-external-k19 — brief

## Goal

The count sees the macOS guests that run outside the app but under the same
user. A `tart run` outside the app then holds a slot, and a start waits without
a refused `tart run` while that guest runs.

## Context

- Spec `docs/specs/admission-queue.md`. Read these sections:
  - "The count", all of it, with "An answer newer than the join".
  - "The admission decision", for the decision at a deadline.
  - "External occupancy".
- Meet the requirement "External occupancy counts".
- ADR-0022. The tart facts are in its context: the list and the lock of tart, and
  the OS of a VM that only `tart get` reports.
- `queue-refusal-k17` handles a guest that the app cannot see through a refusal.
  This leaf adds the guests that it can see.

## Done when

- One observer makes each observation, one at a time. It numbers them in the
  order of their start, and the requests that wait together share one. It
  observes before each decision except the two that the spec names, and every
  3 s while the queue holds a waiter. It never holds the lock of the queue while
  tart runs.
- An observation runs `tart list`, and `tart get` for each VM that runs. Only a
  guest whose OS is `darwin` counts. The app keeps no answer of `tart get` between
  observations. A failed `tart get` makes the app read `tart list` again, as the
  spec states.
- The count adds the seen holders to the own holders. A guest that two holders
  name counts once. A seen holder counts against the two slots and not against
  `macos_slots`.
- An observation fails when a call fails or does not answer within 5 s. The app
  then keeps its last answer. A stop of the app removes its VM from the answer
  only on a later `tart list` that does not name it. The exit of a `tart run`
  removes no VM.
- The app admits nothing until tart first answers in a run. It admits a start only
  on an answer whose observation started after the join. A start without `--wait`
  whose observation fails gets `refused_by` `app`.
- The decision at a deadline waits for no observation. It uses the last answer.
- A waiting start writes a line that says that tart does not answer, with the
  time of the last answer when one exists. The remedy says so too.
- The pure functions that build the count and the details of `VM_LIMIT_REACHED`
  have the tests that the spec lists. They cover these cases:
  - a name that two holders share
  - a VM that tart replaced with a guest of another OS
  - a stop that began after the last answer
  - the exit of a `tart run`
  - a meta whose `tart run` is gone
  - a decision before the first answer
  - the metas of stopped VMs after a restart of the Mac.
- The protocol tests use a fake `tart` with a slow `tart get`, a `tart list` that
  does not answer, and a fake guest that starts outside the app. They cover a
  wait that runs out during a slow observation, and a slot that frees only after
  the deadline.

## Notes

- This leaf proved larger than one session, so it is a node. Two children share
  its "Done when":
  - `queue-observer-k27`: the observer, the seen holders and the count, with the
    answer that is newer than the join and the decision at a deadline.
  - `queue-tart-failure-k28`: the bound of 5 s for a call, and the stop of the
    app that a later read of tart settles. It also holds the exit of a `tart run`,
    the lines and the remedy about a silent tart, and the protocol tests of a slow
    tart.
- The node closes when both children retire. The close compares the "Done when"
  above with the two.
- `observed_at` and `unanswered_since` stay in memory here. `queue-vm-slots-k20`
  shows them.
