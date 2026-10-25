# queue-observer-k27

## Goal

The app counts the macOS guests that run outside it under the same user. One
observer asks tart which guests run. A `tart run` outside the app then holds a
slot, and a start waits without a refused `tart run` while that guest runs and
tart answers.

## Context

- The brief of `queue-external-k19` holds the full "Done when". This child takes
  the first part of it, as the notes of that brief say.
- Spec `docs/specs/admission-queue.md`: "The count" (all of it, with "An answer
  newer than the join"), "The admission decision", "A start without `--wait`" and
  "External occupancy".
- Code: `cli-rs/crates/testanyware-app/src/admission.rs`, `admission/count.rs` and
  `admission/decision.rs`. The tart helpers are in
  `cli-rs/crates/testanyware-vm/src/tart.rs`. The fake `tart` is
  `cli-rs/crates/testanyware-vm/tests/fixtures/fake-tart`. The protocol tests are
  `cli-rs/crates/testanyware-cli/tests/app-vm-queue.rs`.
- `queue-refusal-k17` handles a guest that the app cannot see through a refusal.
  This child adds the guests that the app can see.

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
- An observation fails when a call returns an error or output that the app cannot
  read. The app then keeps its last answer. (The bound of 5 s for a call belongs
  to `queue-tart-failure-k28`.)
- The app admits nothing until tart first answers in a run. It admits a start only
  on an answer whose observation started after the join. A start without `--wait`
  whose observation fails gets `refused_by` `app`.
- The decision at a deadline waits for no observation. It uses the last answer.
- The pure functions that build the count have tests for these cases:
  - a name that two holders share
  - a VM that tart replaced with a guest of another OS
  - the exit of a `tart run`
  - a meta whose `tart run` is gone
  - a decision before the first answer
  - the metas of stopped VMs after a restart of the Mac.
- The fake `tart` gains `get`, which reports the OS of a VM. The protocol test
  runs a fake guest that starts outside the app. A start waits while it runs and
  runs no refused `tart run`.

## Notes

- `observed_at` stays in memory here. `queue-vm-slots-k20` shows it.
- The cases "a stop that began after the last answer" and the slow and silent
  tart of the protocol tests belong to `queue-tart-failure-k28`.

## Decisions (running log)

**The leaf became a node.** `queue-external-k19` held an observer, a count, five failure rules and
the deadline rules. It is now the node `19-k19`. This child takes the observer, the count and the
rules about the join. `queue-tart-failure-k28` takes the failure rules.

**One observer thread, started on the first request.** The thread holds a `Weak` reference to the
queue, so it ends soon after the last reference goes. It makes one observation at a time and runs
tart outside the lock. The tick of 3 s is the thread's own. It observes when the queue holds a
waiter and 3 s passed since the last start.

**A decision follows each observation, and a waiter decides only at its deadline.** The observer
runs `run_decision` after every observation, also after one that failed. An event (a join, a leave,
a release, an acceptance, a refusal, the end of a pause) only asks for an observation. The waiting
thread decides alone at its own deadline, on the last answer, so a slow tart never delays that
decision. The unit test holds `tart list` and shows a wait of 1 s ending at about 1 s.

**An event asks for an observation only while a waiter stands in the queue.** With no waiter, no
decision follows. A `vm stop` in an idle app therefore runs no `tart`. This also keeps the stop
tests free of a `tart list`.

**The number of an observation is a plain counter.** A join notes `next_obs`. An answer is newer
than the join when its number is that value or higher. `decide` takes `answer: Option<u64>` and the
`joined_obs` of each waiter. A refused start keeps the `joined_obs` of its first join.

**A start without `--wait` has three refusals that need no answer.** They are a waiter in the
queue, contention with an unproven start, and admitted starts that fill `macos_slots` or the two
slots. Otherwise the start asks for an observation, polls its cancel every 100 ms, and decides on
the first answer newer than its join. A failed observation after the join gives `refused_by` `app`.

**The count is a pure function.** `count::holders(own, answer)` returns the own and the seen
holders. The seen holders are the guests of the answer that no own holder names. The queue keeps
`own_held` for the `limit` of a refusal, and `holders` (own and seen) for the lines and the details.

**Strict readers in the vm crate.** `parse_running_names_strict` and `parse_os` fail on text that
is not tart's output. The older `parse_running_names` reads such text as "no VM runs". That reading
would free a slot, so the observer does not use it. `SystemTart` is the reader that runs the real
`tart`. A host that is not macOS gets a reader that always fails, because no other host runs tart.

**`with_tools` rebuilds the queue.** The app builds the queue before a test gives it a tool
directory. `AppContext::with_tools` now builds the queue again with the new search.

**The line of a waiter names a seen guest.** The first line comes at the join, from the last
answer, so it can miss a guest that started a moment before. The next observation changes the
holders, and a new line names the guest.

**The fake `tart` gains `get`.** It reports the OS from `<name>.os`. `clone` writes `darwin` for the
macOS golden and `linux` for the Linux golden, and a VM that a test makes by hand reports `darwin`.

**The tests are mutation-checked.** With the seen holders left out of the decision, one unit test
and the protocol test of an external guest fail. With the rule of the newer answer removed, three
unit tests fail. After each run, a `diff` against a saved copy showed the restored source.

**The numbers of the tests.** The app library has 231 tests, and 10 of them are new tests of the
queue. The pure functions gained cases for the count and the decision. The queue protocol suite has
20 tests, and 4 of them are new. The routing suites and the vm crate stay green.

**No document of this scope changed.** The glossary, the contract and the spec already state the
count with seen holders. The Taskfile targets already run the new tests.

**No review leaf, and the in-session allowance stays unspent.** The claim that the compiler cannot
check is the lock discipline of the observer. The tests cover it with a held `tart list`, and the
two mutations show that the tests catch a wrong rule.

## What the next child inherits

- A hung call to tart blocks the observer thread for ever. Waiters still reach their deadlines,
  because they decide alone. The bound of 5 s per call is the first job of `queue-tart-failure-k28`.
- `observe` drops the reason of a failure. `queue-tart-failure-k28` needs the time of the first
  failure for `unanswered_since` and for the line that says tart does not answer.
