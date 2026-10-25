# queue-refusal-k17

## Goal

A refusal by Virtualization.framework sends the start back to the head of the
queue, with a pause. The app then tries one start at a time. So an unseen guest
costs a retry every 3 s, and no later start takes a freed slot before an
earlier one. This increment also changes the documents, because the order holds
under every refusal from here.

## Context

- `queue-order-k16`, which leaves the retry of today inside the hold of an
  admitted start. This leaf replaces that retry.
- Spec sections "A refusal by the framework", "The wait and its end" and "A start
  without `--wait`". Take the requirements "A refusal sends the start back to
  the head" and the three scenarios of "A wait that runs out" that involve the
  pause.
- ADR-0022, and the glossary terms *Contention* and *Unproven*.
- The note of the planning leaf `admission-increments-k3`: the help of `vm start`
  still says that the waiters are not served in order. `cli-contract.rs` checks
  that text, and `docs/reference/cli-commands.md` repeats it.

## Done when

- A refused start loses its hold, goes back to the place of its join, ahead of
  each waiter that joined after it, and starts contention. The app notes the
  time and the line of the refusal.
- After each refusal the app admits nothing for 3 s. The pause ends at the
  deadline of the head at the latest. It never blocks a start without `--wait`.
- In contention the app admits the head alone, and only while no admitted start
  is unproven. Contention ends as the spec states. An acceptance of a start
  that the app admitted before the refusal does not end it.
- Two starts that the app admitted together and the framework refused both go
  back in the order of their join, whichever refusal comes first.
- The pure function gains the last refusal and the pause as inputs. Its tests
  cover these cases:
  - a refusal, and two refusals together
  - an acceptance before the refusal, and the end of a pause
  - an admission of a start beside a golden hold in contention. The golden hold
    is a synthetic holder in the test.
- A start without `--wait` fails at once when contention blocks an admission. It
  fails with `refused_by` `framework` when the framework refuses it after the
  admission.
- A waiting start writes a line after a refusal that says so and names the
  pause.
- The framework can refuse while the count shows a free slot. Then the remedy
  of `VM_LIMIT_REACHED` names two causes:
  - a guest of another user or tool
  - a stuck count of the framework, which only a restart of the Mac clears.
- A protocol test uses a fake `tart` whose capacity is below the count of its
  visible guests, which stands for an unseen guest. Three real starts with
  `--wait` then get their slots in the order of their join.
- The help of `vm start` drops the line about waiters that are not served in
  order. It says that the app serves them in order. `cli-contract.rs` and
  `docs/reference/cli-commands.md` change with it.
- `docs/architecture/cli-design-contract.md` changes the `--wait` row, the
  `VM_LIMIT_REACHED` row and the amendment of 2026-10-19. It gains the
  amendment for the queue. The contract and the app spec state that one user per
  Mac is the supported deployment.

## Notes

- The help does not name `vm slots` yet. `queue-vm-slots-k20` adds that name to
  the help and to the remedy.

## Decisions (running log)

**One hold object lives through the refusal.** `SlotHold` gains `accepted(&self)` and
`refused(&mut self, line, cancel, progress)`. The trait is now `Send + Sync`. The hold ends
at the refusal, and the queue may give it a new admission. So the request id, the join time
and the place of the join stay the same.

`refused` returns `Readmission::Admitted` when the app admitted the start again. It returns
`Readmission::Stands` when no wait is left. A cancel is `StartCancelled`. A wait that runs
out in the queue is `VM_LIMIT_REACHED` with `refused_by` `app`, and `tart_error` holds the
last line of tart.

**The framework accepts at the VNC URL, so the hook sits there.** `observe_tart_boot` calls
`hold.accepted()` when `await_vnc_url` finds the URL. A later hook would hold contention
for the whole boot of the guest, which is minutes.

**The retry of today stays for the direct path only.** `run_until_slot` takes the queue path
when a hold exists (`run_in_queue`) and the old `wait_for_slot` loop otherwise. The old
loop and its reporter are not removed: a start that has no gate still uses them.

**The pure function and its two helpers.** `decide` gains `unproven`, `contention`,
`last_refusal` and `pause`. The pause ends at the deadline of the head, so the head always
gets its last try. In contention it admits the head alone, and only while no admitted start
is unproven. `admits_without_wait` gains the contention input.

`contention_after_settle` and `contention_after_acceptance` are pure too. So two rules have
unit tests. Contention ends when nothing waits and nothing is unproven. An acceptance of a
start that the app admitted before the refusal does not end it.

**A start without a wait that the framework refuses also records the refusal.** The spec
says nothing about contention for such a start, because it cannot go back into the queue.
It still proves a guest that the app cannot see. So the refusal starts the pause and
contention in the same way, and contention ends at once when nothing waits and nothing is
unproven. The decision of a start without `--wait` ignores the pause by its own rule.

**`unseen_possible` is not a `details` member.** The spec lists no new member, and the
brief says that no `--json` field changes its meaning. The flag lives in `AdmissionFacts`.
It is true for a refusal by the framework after an admission. It is also true for a refusal
by the app while contention holds, or after a refusal of the same start.

Only the remedy reads the flag. The app builds the remedy, so it crosses the protocol as
text.

**Two lines for a refusal, and each repeats only when something changed.** The refusal line
names the request, the line of tart, the place and the pause of 3 s. It repeats on a new
place, a new line of tart or after 30 s. The wait line of the k16 queue stays. Its
reporter now lives in the hold, so a retry every 3 s does not print it again.

**The pause has its own wake.** A refusal raises no event 3 s later, and the tick of 3 s can
miss by up to 3 s. `wait_admitted` decides again when the pause has ended and no decision
has run since.

**The tests are mutation-checked.** With the queue position of a refused start changed to
the back of the queue, three tests fail. With the start of contention removed, five fail.
With each acceptance ending contention, the test of the acceptance before the refusal fails.
A first try at the second mutation used the GNU form of `sed`, which does nothing on macOS,
and so showed all green. The file was compared with its copy before the run was credited.

**A flaky test of k16 is fixed.** `a_cancel_moves_the_places_behind_it_up` cleared the
guests at once after the cancel. A busy machine could admit the second waiter before it
printed "place 1". The test now waits for that line first. It passed 60 of 60 runs alone and
failed once under load before the fix.

**The documents of this scope.** The help of `vm start`, `cli-contract.rs`,
`docs/reference/cli-commands.md`, `docs/reference/error-codes.md`, the `--wait` row, the
`VM_LIMIT_REACHED` row and the amendments of 2026-10-19 and 2026-10-25 in the contract, and
the app spec. The contract and the app spec now state that one user per Mac is the supported
deployment. The help does not name `vm slots` yet, as the brief says.
`website/docs/reference/cli-commands.md` has no `--wait` entry at all and is not part of
this leaf.

**No review leaf, and the in-session allowance stays unspent.** The lock discipline is the
claim that the compiler cannot check. Tests cover the cancel race of k16 and the cancel of a
start that the queue took back. The mutations show that the tests catch a wrong queue.
