# queue-record-retry-k30

## Goal

A record that tart did not decide is decided later. The app keeps the VM and the
record while tart does not answer. It decides again after each observation whose
`tart list` answered. It never stops a VM that no meta proves.

## Context

- Spec `docs/specs/admission-queue.md`, the section "A quit, a crash and a
  restart". Read from "A record that tart did not decide" to the end of the
  section. Read the scenarios of the requirement "A crash or a restart keeps the
  count true", and the runs of that case in "Test seams".
- The brief of `queue-start-record-k23` holds the full "Done when".
- `queue-record-decision-k29` wrote the record, the function of the four rows and
  the decision before the socket is served. Read its decision log first.
- `queue-external-k19` built the observer. This child reuses it.

## Done when

- The observer decides again for each record that the fourth row kept. It decides
  after each observation whose `tart list` answered. That includes an observation
  whose later call failed. It decides after the app keeps that answer, and before
  the next observation starts. The tick of 3 s makes such an observation while no
  client asks for one.
- The observer tries the gate of the VM id, as a stop takes it, and never waits for
  it. When a start or a stop of the id holds the gate, the record waits for the next
  list. A start of the id replaces the record in any case. The observer holds no
  lock of the queue while it holds a gate.
- When a list reports the VM running, the app notes that for the record, in memory.
  The third row then decides at the first decision that gets the gate. Later lists
  change nothing.
- The second row never comes from the list of the observation. Suppose no list of
  this run reported the VM running. Then the observer reads `tart list` again under
  the gate, with the bound of 5 s, and decides with that read. When the read does
  not answer, the record stays.
- The decisions delay the next observation but never the decision at a deadline.
- `GET /v1/status` reports each new result.
- The tests of a decision again cover the lists of observations:
  - a list that reports the VM running gives the third row
  - a later list that does not report it gives the third row too
  - a list that does not report it, with no such list before it, never gives the
    second row.
- A protocol test replaces the clone of a record with another VM while the app is
  down. It runs with a `tart list` that does not answer at the next start. The tick
  of the app makes the observation, and the app decides again.
- Two more runs of that case pass:
  - Each later call to tart fails for a while. Then the VM stops, and the VM stays.
  - A `vm start --wait` of the same id joins the queue while the record stays. The
    queue serves its waiters, and `vm slots` answers.

## Notes

- The note in memory lives as long as the run of the app. After a crash, the next
  start decides again (spec, "Open risks").

## Decisions (running log)

**The queue calls one hook after each observation whose `tart list` answered.** `AfterObservation`
(`admission.rs`) has `pending()` and `after(listed)`. The observer calls `after` once the answer is
kept and the decision has run, and it drops the lock of the queue first. So a join, a view and a
decision at a deadline never wait for a hook that reads `tart` under a gate. A test blocks the hook
and runs a wait to its deadline meanwhile (`a_hook_that_works_holds_no_lock_of_the_queue`).

**`observe_listed` returns the last list that answered, also when a later call failed.** `observe`
stays as the old function over it. A failed `tart get` re-lists, and the re-list replaces the list
only when it answers. A list that failed gives no list, and the hook does not run.

**The tick also observes while the hook has work.** The condition was "a waiter stands". It is now
"a waiter stands, or the hook is pending". `set_after_observation` starts the observer when the
hook has work at once, because the thread starts only on a request. A hook without work makes no
observation (`a_hook_without_work_makes_no_observation_while_no_client_waits`).

**The retry keeps the records in memory, from the fourth row of the start.** `run_keeping` returns
the records whose row was the fourth, and `run` stays as it was. Only those records are decided
again. The records of the starts of this run are never in the set, so the retry cannot touch a live
start.

**The attempt for one id: note, gate, compare, state, settle.** (1) A list that reports the VM
sets the note, whatever the gate says. (2) `StartGate::try_acquire` never waits. (3) Under the gate
the app loads the file. The record that it kept decides. A different record, or none, means that a
start of the id replaced it or ended. (4) With the note, the state is `Running`. Without it the app
reads `tart list` once more under the gate, and the second row can only come from that read. (5)
`settle_tart` applies the verdict. A read that fails leaves the record for the next list.

**A record that a start replaced gets a last result.** The spec says that a start of the id
replaces the record in any case, and it says nothing about the status. The result of the fourth row
would say "decides again" for ever, so the retry reports `removed` false and says that a later
start replaced the record. The VM of the id belongs to that start's clone step.

**A record that cannot be read stays.** Nothing says that a start replaced it, so the retry drops
nothing and reports nothing. The next start of the app reports it as it does today.

**A removal that fails drops the record from the set, not from the disk.** The spec says the next
start of the app decides again. A retry on every list would run `tart delete` every 3 s.

**The hook holds the context weakly.** The context owns the queue and the queue owns the hook. The
status result goes through `AdoptionLog::update_unfinished_start`, which replaces the result of the
same id.

**The fake `tart` gets `get-fails`.** It makes `tart get` fail while `tart list` answers. The run
"each later call to tart fails" needs that. The other tests never set it.

**The tests discriminate.** The two protocol runs of the case timed out when the call to `watch`
in `main` was replaced by a no-op, and they passed with it restored. The unit tests are 12 in
`unfinished.rs`, 4 in `observation.rs`, 2 in `gate_start.rs` and 6 in `admission.rs`. The
protocol tests are 3 in `app-vm-queue.rs`. `task app:test` and `task app:routing-test` pass.

**Left to the next leaf.** The quit of the app and its record of a start that the 10 s leave
unfinished belong to `queue-quit-k31`. A record that this leaf kept in the set survives only as long
as the run, and the next start of the app decides for the file again.
