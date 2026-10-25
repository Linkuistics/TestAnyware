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
