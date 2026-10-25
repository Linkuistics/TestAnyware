# admission-queue-k10

**Reviews:** admission-queue-k9

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k9`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k9`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k8` and of one reviewer that
  k9 ran.
- `docs/specs/admission-queue.md`: "Solution", "The count" with "When tart does
  not answer" and "Before the first answer", and "The admission decision".
  Also the start record of "A quit, a crash and a restart", with both of its
  tables, "A restart" and the flowchart. Also the details and the remedy of
  `VM_LIMIT_REACHED`, the stderr lines, the `vm slots` table, the test seams
  and the open risks.
- `CONTEXT.md`: *Holder*, *Contention*, *Start record* and *Adoption*.
  ADR-0022, and the amendment of 2026-10-25 in
  `docs/architecture/cli-design-contract.md`.
- The code of today: the adoption decision and its scan, `stop_app`,
  `stop_proven`, `stop_without_signal`, `TartRunner::prepare_in` and `stop_in`.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model of the queue. So these rules rest on prose and
on the seam of pure functions, and no test exists for them yet. This is the
fourth review of the design, after k4, k6 and k8. Report a case only when it is
worse than the spec admits.

**The own holders.** k9 makes a running guest of a meta an own holder unless
the check of the meta's `tart run` finds it gone or finds another process. A
meta that records no identity still counts. Check a VM that the app started in
this run, and an adopted VM. Check a legacy VM and a meta written with no
identity. Check
also a person's VM under the name of a stale meta, and `vm stop` of that id.

**Before the first answer.** k9 admits nothing until tart first answers in a
run of the app. Check a start without `--wait`, `--wait=0`, a golden creation as
the only waiter, and `macos_slots` 0. Check that some event decides again once
an observation answers.

**The kept observation.** While observations fail, the app keeps the seen
holders of its last answer. It removes each VM that the app stopped, or whose
`tart run` it saw exit, after that answer. Check that no order of events makes
the count too low. Check also how long a count that is too high can last.

**The OS of a VM.** The app now asks `tart get` for the OS of each running VM
at each observation, and it keeps no answer. Check a VM that disappears between
`tart list` and `tart get`, and a `tart get` that fails in another way.

**The start record.** Row 3 now keeps the spec and the meta only when the
meta's own `tart run` matches in full. A table states the result and `removed`
for each outcome of the check. Check each outcome against the requirement that
the app never stops a VM that it did not start.

**The trade-offs that k9 accepted.** A tart that does not answer can make a
waiter fail beside a free slot, and a golden creation wait. An orphan VM of the
app counts against the two slots but not against `macos_slots`. Report one of
these only when the case is worse than the spec admits.

**The sets.** The spec, ADR-0022, the contract's amendment of 2026-10-25 and
`CONTEXT.md` must agree with each other.
