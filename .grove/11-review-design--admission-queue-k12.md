# admission-queue-k12

**Reviews:** admission-queue-k11

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k11`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k11`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k10` and of one reviewer
  that k11 ran.
- `docs/specs/admission-queue.md`: "The count" with "When tart does not answer"
  and "Before the first answer". Also the start record of "A quit, a crash and a
  restart", with both of its tables, "A record that tart did not decide" and the
  flowchart. Also the `vm slots` table, the scenarios of "A crash or a restart
  keeps the count true", the test seams and the open risks.
- `CONTEXT.md`: *Start record* and *Holder*. The start record in "Lifecycle and
  adoption" of `docs/specs/testanyware-app.md`.
- The code of today: `vm_exists_in`, `TartRunner::stop_in` and `prepare_in`,
  `stop_app`, `stop_proven`, `stop_without_signal`, the adoption decision and
  its scan, the stop route, and the reaping of detached children.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

This is the fifth review of the design, after k4, k6, k8 and k10. The human
asked for focus, so report a case only when it is worse than the spec admits.
The human rejected a Quint model of the queue, and no test exists for these
rules yet.

**The retry of the fourth row.** When tart does not answer at the start of the
app, the app now decides again for the record after each observation that tart
answers. That decision runs while the app serves its socket, under the gate of
the VM id.

Check the retry beside a `vm stop` and a new `vm start` of the same id, a quit,
and a crash during the retry. Check a tart that answers an observation but not
the retry. The third row at the start of the app would keep some VMs.
Check that the second row of a retry never removes one of them.

**The read of a stop.** A stop now removes a VM from the kept answer only
through its last `tart list`. That list must answer without the VM. It must
also start after the last call of the observation returns.

Check that order against concurrent observations and stops. Check that the read of a stop changes neither `observed_at` nor
`unanswered_since`, and is never the first answer. Today `vm_exists_in` reads a
failed `tart list` as no VM. Check that the spec makes the implementer close
that gap.

**The exit.** The exit of a `tart run` now ends only the hold of the admission
that started it, and it removes no VM from the kept answer. Check a start that
the framework refused and that runs `tart run` again on its clone.

**A failed `tart get`.** The app now reads `tart list` again after a failed
`tart get`. Check what a `tart get` that keeps failing does to the count and to
the waiters.

**QEMU.** The table of the start record now decides for a start on tart only.
Check that no row can reach a start on QEMU.

**The trade-off that k11 made visible.** A meta that outlives its `tart run`
lets `vm stop` remove a VM of that name. k7 accepted that hazard of adoption.
Report it only when a case is worse than the new open risk states.

**The sets.** The spec, `CONTEXT.md`, the app's spec, ADR-0022 and the
amendment of 2026-10-25 in `docs/architecture/cli-design-contract.md` must agree
with each other. k11 changed neither ADR-0022 nor the contract.
