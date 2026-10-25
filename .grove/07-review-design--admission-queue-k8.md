# admission-queue-k8

**Reviews:** admission-queue-k7

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k7`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k7`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k6` and of one reviewer that
  k7 ran.
- `docs/specs/admission-queue.md`: "The count", "The admission decision", "A
  refusal by the framework" and "A start without `--wait`". Also "The wait and
  its end", "Golden creation", "A quit, a crash and a restart" and "External
  occupancy". Also the details of `VM_LIMIT_REACHED`, its requirements, its
  test seams, its open risks and its "Out of scope".
- `CONTEXT.md`: *Contention*, *Unproven* and *Start record*. ADR-0022, and the
  amendment of 2026-10-25 in `docs/architecture/cli-design-contract.md`.
- The code of today: `TartRunner::prepare_in`, the adoption decision,
  `stop_proven` and the meta write of a tart start.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model of the queue. So these rules rest on prose and
on the seam of pure functions, and no test exists for them yet.

**The end of contention.** k7 ends contention only at an acceptance of a start
that the app admitted in contention. Contention also ends when the queue holds
no waiter and no start is unproven. Look for an order of events that holds
contention with no end. Look for one that lets two starts try together while it
holds. Look also for one that blocks a start that the limit does not block.

**The waiter.** k7 changed "a start waits" to "the queue holds a waiter" in five
rules. They are the tick, the observation, the decision, the end of contention
and a start without `--wait`. Check each rule with a golden creation as the
only waiter. Check also a golden creation that waits while the app quits or
crashes.

**The count when tart is slow.** The app now keeps its last observation that
tart answered. Before the first answer, it counts each VM of a meta as own.
Check this against "a full count is a fact", `macos_slots` and the hold of
golden creation. Check also a stale observation that hides a free slot.

**The start record.** Row 3 now removes the spec and meta sidecars, unless
adoption adopts the VM by its meta. Check each outcome of adoption, and a
second start of the same id that crashes before its clone step. Check the
report of `removed` for each case.

**The error details.** `refused_by` `app` now covers a full count and a try
that contention held back. Check the remedy, `limit` and `holders` for each
cause.

**The trade-offs that k7 accepted.** A golden creation beside a guest that the
app cannot see can fail. A waiter behind the head can fail at a deadline during
the pause. A `vm start --id` removes a VM of that id, as today. Report one of
these only when the case is worse than the spec admits.

**The sets.** The spec, ADR-0022, the contract's amendment of 2026-10-25 and
`CONTEXT.md` must agree with each other.
