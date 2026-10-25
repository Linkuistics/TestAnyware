# admission-queue-k4

**Reviews:** admission-queue-k2

## Goal

Read the design of the admission queue adversarially, and report findings
without fixes. The design is `docs/specs/admission-queue.md` and ADR-0022. It
also includes the edits that `admission-queue-k2` made to ADR-0021, to the
contract, to the app's spec and to `CONTEXT.md`.

## Context

- The commit of `admission-queue-k2`, and the decision log in its task file.
- `docs/specs/testanyware-app.md`: "The protocol", "Lifecycle and adoption" and
  "Version skew", which the queue extends.
- `docs/architecture/cli-design-contract.md`: §3.2 and the `VM_LIMIT_REACHED`
  row of §4.2. Also the amendments of 2026-10-19 (`--wait`) and 2026-10-25.
- The code of today: `run_until_slot` and `wait_for_slot` in the VM lifecycle
  crate, and the app's start handler with its gate for each VM id. Also
  `provision_boot1`, and the recovery cycle of ADR-0008.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model, because one process behind one lock runs the
queue. So the claims about a cancel and a crash rest on prose. The producer
could not settle these doubts alone.

**Every interleaving.** The events are a join, a leave, an admission, a refusal,
an acceptance, a release, an expiry and a quit. Look for an order of them that
loses a slot or leaves a VM. Look also for one that frees the slot of a live
guest. These cases need the closest look:

- a cancel at the admission
- a quit during a boot
- a crash between the removal of the start record and the result
- a quit while golden creation holds its slot.

**Contention.** Can the rule of one start at a time block starts that the limit
does not block? Can a start that the framework never answers hold the
contention open? The spec relies on the VNC stage of the boot to end such a
start.

**The count.** Check the rule for a guest that two holders name. Check an
adopted VM of an older meta. Check the fall-back to the own holders when tart
does not answer within 5 s.

**Compatibility.** Check each new member of `VM_LIMIT_REACHED` against §3.2, and
the empty `tart_error`. Check the rules of protocol version 2 against an older
Host CLI and an older app. An older Host CLI fails on an event that it does not
know, so check that the queue adds none to `POST /v1/vms`.

**The sets.** The spec and the app's spec must not restate each other. ADR-0022
and ADR-0021 must be a minimum coherent set. The contract's amendment of
2026-10-25 must agree with the spec.
