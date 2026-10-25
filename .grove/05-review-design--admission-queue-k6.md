# admission-queue-k6

**Reviews:** admission-queue-k5

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k5`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k5`, and the decision log in its task file. The
  log triages the findings of `admission-queue-k4` and of one reviewer that k5
  ran.
- `docs/specs/admission-queue.md`: "The order", "The admission decision", "A
  refusal by the framework", "A start without `--wait`", "The wait and its end"
  and "A quit, a crash and a restart". Also its requirements, its test seams and
  its open risks.
- `CONTEXT.md`: *Contention*, *Unproven* and *Start record*.
- The code of today: `run_until_slot` and `wait_for_slot` in the VM lifecycle
  crate, and `next_slot_step`. Also the adoption decision, `stop_proven`, and
  the point where a start writes its meta.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model of the queue. So these rules rest on prose and
on the seam of pure functions, and no test exists for them yet.

**Contention.** k5 made the pause after a refusal hold apart from contention. It
defined an unproven start. It also ends contention when no start waits and none
is unproven.

Look for an order of events that lets two starts try together after a refusal.
Look for one that blocks a start that the limit does not block. Look also for
one that holds contention open with no end. These cases need the closest look:

- a golden creation at the head in contention
- a start that ends with `VM_BOOT_TIMEOUT` while its `tart run` lives
- an acceptance while a refused start waits.

**The deadline.** The decision admits first, and then it fails each waiter whose
deadline came. Check a deadline during the pause, and a deadline while another
start is unproven. Check `--wait=0` against a start without `--wait` in each
state of the queue.

**The start record.** A table decides by the identity check and by the state
that tart reports for the VM. Check each row against "the app never stops a VM
that it did not start" and "no slot is lost". Check the two open risks that k5
accepted:

- a VM that does not run, under the name of a start record
- a crash before the start record names its `tart run`.

**The sets.** The spec, ADR-0022, the contract's amendment of 2026-10-25, the
app's spec and `CONTEXT.md` must agree with each other.
