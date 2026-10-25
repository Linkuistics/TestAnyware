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

## Decisions (running log)

**The bound lives in the vm crate, and it kills the whole process group.** `run_tart_bounded_in` in
`tart.rs` spawns `tart` as the leader of its own process group, polls it every 10 ms, and on expiry
sends SIGKILL to the group. A kill of the child alone would leave a helper that holds the pipes, and
the reader threads would then wait for it. `running_names_strict_in` and `os_of_in` take the bound
as an argument. `SystemTart` carries it, and its default is `CALL_BOUND`, 5 s. The lenient readers
that `vm list` uses stay unbounded, because the spec bounds the calls of an observation only.

**A killed call cannot answer late.** The call ends with its process, so "a call that answers after
its observation failed changes nothing" holds by construction. The time of an answer is the moment
when `observe` returns, which is the return of its last call.

**The read of a stop is its own `tart list`, made after the stop.** `AdmissionQueue::stopped`
replaces `holder_ended` in `stop_vm_with`. It reads the last answer under the lock, and it makes no
read when that answer does not name the VM. So a `vm stop` in an idle app still runs no `tart` of
the queue. Otherwise it notes the start time, runs `tart list` outside the lock, and applies
`count::stop_read_removes`. That pure function removes the VM only when the read started after the
answer ended, the read answered, and the read does not report the VM running. A stop read without
this much proof changes nothing, and the count stays too high rather than too low.

**The read uses "not running", where the spec says "does not name".** The reader of the queue
returns the running VMs only, and a VM that exists but does not run holds no slot. A stop deletes
the VM, so for a stop the two readings agree.

**A removal lives inside the answer.** `Answer.macos` loses the name, and a newer answer replaces
the whole value. So a newer answer that still reports the VM running counts it again, as the spec
states. After a removal, `stopped` runs the decision at once, so the slot frees in the count with
no further observation. The read changes neither `Answer.at` nor `unanswered_since`.

**`unanswered_since` is a field of the state.** The first failed observation sets it when it is
empty, and the next answer clears it. `unanswered_since()` reads it for `vm slots` in
`queue-vm-slots-k20`. It stays in memory, on the clock of the queue.

**The exit of a `tart run` needed no code.** `release` removes the admitted start only, and the
answer is untouched. A queue test now shows this with a replacement under the same name and failed
observations after the exit.

**The silence reaches the remedy through `AdmissionFacts.tart_silence`.** It is no `details` member.
`silence()` says that tart did not serve a start in two cases: an observation failed since the last
answer, or no answer is newer than the join of the start. The second case covers a slow tart, where
nothing failed. The remedy says so and names the seconds since the last answer, or that tart has not
answered since the app started. A refusal by the framework gets the first case through the new
`SlotHold::tart_silence`, whose default is `None`.

**The lines say it only while observations fail.** A waiting line and the line after a refusal add
"tart does not answer" with the age of the last answer. The waiting line repeats when the silence
begins or ends, as it repeats when the place or the holders change.

**The fake `tart` gains `list-hang` and `get-delay`.** `list-hang` makes `list` wait while the file
exists, so a test can release a call that the bound did not kill. `get-delay` is a number of
seconds.

**The tests are mutation-checked.** Without the "started after the answer" rule, one pure test
fails. With the removal left out of `stopped`, two queue tests fail. With `release` also removing
the VM from the answer, the test of the exit fails. With a bound of 600 s, the protocol test of a
silent `tart list` fails. After each run a `diff` against a saved copy showed the restored source.

**No review leaf, and the in-session allowance stays unspent.** The claim that the compiler cannot
check is the rule of the later read. A pure function holds it, and the mutations show that the tests
catch a wrong rule.

## What the close of the node found

- The cleanup of a start that the queue refused runs `tart list` with no bound (`abandon_tart` calls
  `TartRunner::stop_in`, and the start calls `vm_exists_in` before its clone). Against a `tart list`
  that never answers, the app decides at the deadline, but the client waits for that cleanup. The
  Done-when bounds the calls of an observation only, so this leaf left them. A protocol test
  releases the hang after the deadline for this reason.
- `vm stop` now waits for one more `tart list` (at most 5 s) when the last answer names the VM.
