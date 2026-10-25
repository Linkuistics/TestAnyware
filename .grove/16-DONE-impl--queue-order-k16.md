# queue-order-k16

## Goal

A `vm start --wait` of a macOS guest waits in one ordered queue in the app. The
app serves the waiters in the order of their join while its own count limits the
starts. This is the first increment, and it is the one that ends the race of
issue #7.

## Context

- Spec `docs/specs/admission-queue.md`. Read these sections:
  - "What takes a slot" and "The order".
  - "A start without `--wait`", "The wait and its end" and "A cancel".
  - "A quit, a crash and a restart", for the quit and the restart only.
  - "The count", for the own holders only.
  - "The admission decision", without the pause and contention.
- Meet these requirements of the spec:
  - "The app serves the waiters in order".
  - "The queue blocks only what the limit blocks".
  - "A start holds its slot until its guest is gone".
  - "A cancel leaks no slot and leaves no VM".
  - "A wait that runs out says that it waited".
- ADR-0022, and the glossary terms *Slot*, *Admission queue*, *Admit* and
  *Holder*.
- The retry of today is `wait_for_slot` in `testanyware-vm/src/lifecycle.rs`. The
  app runs `vm start` through `testanyware-app/src/start.rs`. The direct path
  must keep working on hosts without the app. Find the seam where the app gives
  a start its admission gate and the direct path gives none.
- The routing tests `app-vm-start-routing.rs` hold the fake `tart` and its
  limit. The root brief names the three test seams.

## Done when

- One pure function decides which waiters the app admits. Its inputs are the
  queue, the own holders, the slot limit of 2 and the time. Its tests cover a
  join, a leave, an admission, a release, a tick, a deadline and `--wait=0`.
  They also cover the last try at a deadline.
- The own holders are two groups. One group is each running macOS guest that
  has a meta and passes the check of adoption. The other is each start that the
  app admitted, until its guest is gone. The count has no other input here.
- Each macOS start gets a request id such as `r-1a2b3c4d`. The id stays the same
  for the whole life of the request, and the app does not reuse it while it
  runs.
- A start joins the queue when its clone is ready. The app admits only the head.
  When the count shows two free slots, the app admits two starts at once. A
  Linux or Windows start never joins.
- A start holds its slot from its admission until its guest is gone. A stop of
  the guest admits the next waiter. A failed or cancelled start releases the
  slot only after its `tart run` exits.
- A cancel behaves as each row of the table in "A cancel" states. The tests
  cover a cancel while the start waits, at the admission and during the boot.
  No test leaves a slot held or a VM behind.
- A wait that runs out fails with `VM_LIMIT_REACHED`, exit 6 and `refused_by`
  `app`, after one last decision at the deadline. The details gain
  `request_id`, `place`, `waited_secs` and `refused_by`. A refusal by the
  framework after the admission gives `refused_by` `framework`. `limit` is 2.
- A start without `--wait`, or with `--wait=0`, never passes a waiter. It fails
  at once when the queue holds a waiter or the admitted starts fill the slots.
  It runs no `tart run` then.
- A waiting start writes a line to stderr when it joins, when its place or the
  holders change, and every 30 s otherwise. The line names the request id, the
  place, the number of waiters and what holds the slots.
- A quit of the app ends each waiting start with `APP_UNREACHABLE` and cause
  `quitting`. After a restart the queue is empty, and the adopted guests count.
- An admitted start that the framework refuses keeps the retry of today inside
  its hold. This is a deliberate handoff state. `queue-refusal-k17` replaces it.
- A protocol test starts the app in its test mode and drives it with three real
  Host CLI processes at once. The fake `tart` has a capacity of 2. The third
  start waits and gets its slot in order after a stop. The test task of the
  Taskfile lists any new test target.
- The existing help text of `vm start` and its test stay as they are. They
  change in `queue-refusal-k17`, where the order holds under every refusal.

## Notes

- Keep the direct path as it is. The retry of today may stay there for hosts
  that run without the app.
- If this leaf proves bigger than one session, decompose it. The first child is
  the pure decision with the own count, wired for a start that waits.

## Decisions (running log)

**The seam is a trait in the vm crate.** `testanyware-vm/src/slot_gate.rs` holds
`SlotGate` and `SlotHold`. `StartContext.gate` is `None` on the direct path and set
by the app. `start_tart` asks the gate after the clone and before the first
`tart run`, and only for `Platform::Macos`. A Linux start on tart never asks.

**One pure function, and a second for the start without a wait.** `decide` in
`admission/decision.rs` takes the queue, the number of own holders, the slot limit
and the time. It admits before it expires, so the deadline gets its last try.
`admits_without_wait` is the one decision of a start without `--wait` and of
`--wait=0`. A start with `--wait=0` gets no decision at a deadline.

**No thread of the queue.** One mutex guards the waiters, the admitted starts and
the verdicts. Each waiting thread decides in its turn: on an event, on the tick of
3 s, and at its own deadline. A decision leaves a verdict for each waiter it
admits or expires. So a start that nobody serves leaves no state, and a quit of the
app leaves no waiter.

**The count has two own groups, and a guest counts once.** The queue keeps the
admitted starts by VM id. `admission/count.rs` reads the metas with the check of
adoption. The union is by VM id, so a start that wrote its meta and has not yet
dropped its hold counts once. A tart meta whose spec does not read counts as macOS,
because a count that is too high frees the slot of no live guest.

**The hold ends in the vm crate, not in the app thread.** The hold is a local of
`start_tart`. A failed or cancelled start drops it after the cleanup, so after
`tart run` has exited. A successful start drops it after the meta is written, and
the meta holds the slot from then on. `vm stop` calls `holder_ended` so that the
next waiter does not wait for the tick.

**The retry of today stays inside the hold.** After the admission the start gets
the wait that is left, `opts.wait` minus the time in the queue. A refusal by the
framework then fills `request_id`, `place`, `waited_secs` and `refused_by`
`framework`. `queue-refusal-k17` replaces this retry.

**The details are in `LimitReached.admission`.** The direct path gives
`request_id` and `place` as `null`, `waited_secs` 0 and `refused_by` `framework`.
`docs/reference/error-codes.md` names the new members. The help, the contract and
`cli-commands.md` stay for `queue-refusal-k17`, as the planning leaf decided.

**A quit needed no new code.** The connection of each waiting start closes with
the app, and the client already maps that to `APP_UNREACHABLE` with cause
`quitting`. A protocol test shows it. The wait of up to 10 s for the cleanup of
admitted starts belongs to `queue-start-record-k23`.

**The tests are mutation-checked.** With the gate removed from `start_response`,
ten of the eleven first protocol tests fail. The one that passes is the case of two
free slots, which needs no queue. The fake `tart` gained a `capacity` file and a
`refused.log`, so a test can show that the framework refused no start.

**No review leaf, and the in-session allowance stays unspent.** The queue is small.
A race test of the cancel against a release, and the mutation check, cover the
claims that the compiler cannot.
