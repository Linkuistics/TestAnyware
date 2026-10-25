# admission-queue-k14

**Reviews:** admission-queue-k13

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k13`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k13`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k12` and of one reviewer
  that k13 ran.
- `docs/specs/admission-queue.md`: "The count" with "When tart does not
  answer", the lock of "The admission decision", and the gates of "A cancel".
  Also "A record that tart did not decide", the hold of golden creation and
  `POST /v1/slots`. Also the new scenario of "External occupancy counts", the
  test seams and the open risks.
- The code of today: `StartGate` and the start thread that holds its permit,
  the stop route, `vm_exists_in`, and the IPSW build of a base.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

This is the sixth review of the design, after k4, k6, k8, k10 and k12. The
human asked for focus, so report a case only when it is worse than the spec
admits. The human rejected a Quint model of the queue, and no test exists for
these rules yet.

**One observer.** The app now makes one observation at a time. A request that
comes during an observation waits for the next one, and the requests that wait
together share it. A newer answer replaces the last answer whole, with each
removal of a stop. Check that no older `tart list` can become the last answer.
Check a stop that overlaps an observation, and an observation that fails late.

**The retry on the observer.** The observer now runs each retry of a
fourth-row record after an observation whose `tart list` answered. It tries
the gate of the VM id and never waits for it. A start of the id holds that gate
while it waits in the queue. Check for a deadlock with a start, a stop and the
queue lock. Check a start of the id that replaces the record.

**The note of a running VM.** A list that reports the VM running leaves a note
in memory. The third row then decides at the first retry that gets the gate,
whatever later lists report. The second row needs a newer read under the gate,
and only when no list of the run reported the VM running. Check a gate that a
stop holds when such a list comes. Check a quit and a crash between the note
and the decision.

**The tick.** The app now observes every 3 s while a fourth-row record stays.
Check that the tick ends when the record goes, also when a start of the id
replaces it.

**Smaller changes.** The bound of 5 s now applies to each call to tart. A
golden hold names each VM that golden creation boots, `<base>-building` of an
IPSW build included. Check that the per-call bound keeps the wait of a waiter
bounded. Check that a person's run of the finished base still counts.

**The sets.** k13 changed only the queue spec. The glossary, the app's spec,
ADR-0022 and the amendment of 2026-10-25 in
`docs/architecture/cli-design-contract.md` must still agree with it.
