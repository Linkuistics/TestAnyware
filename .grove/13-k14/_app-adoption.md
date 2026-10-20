# app-adoption-k14 — brief

## Goal

Keep VMs alive across the app, and let the app serve them again when it starts. The app signals a process only when it proves the identity of that process.

## Done when

- The meta of a VM records the pid, the start time and the boot session UUID of each process that it names. The meta of a QEMU guest also records its session directory.
- When the app starts, it reads each sidecar and decides for each VM: adopt, report as stopped, or report as not adopted. It signals a process only when all three identity values match.
- The app serves its socket before it probes the survivors.
- An older meta, with no identity, gets streams only. The app stops such a guest by name or through its control sockets, and never by a signal.
- A failed adoption never stops, kills or starts a VM.
- A live check shows that a quit and a new start of the app keeps each VM.

## Decomposition

1. `vm-process-identity`: record the identity, and start swtpm so that it ends with QEMU.
2. `qemu-legacy-stop`: the stop of an older QEMU guest. It needs no identity, so it comes before the guard.
3. `app-adoption`: the decision at start and the signal guard.
4. `adoption-live-check`: the live proof.

## Ordering rule

The guard must never leave an older QEMU guest without a safe stop. The existing QEMU stop signals the process and deletes the clone without proof. So the safe stop of an older guest lands in step 2, before the guard of step 3 exists. Every step ends with a stop that is either as safe as today or safer.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", and the requirement "VMs outlive the app".
- ADR-0017, ADR-0018 and ADR-0004.
- Glossary term: Adoption.
- The decision log of `vm-broker-design-k6`, finding F3.

## Notes

- Test the decisions as pure functions with injected process probes. A fake VM backend is out of scope, by the choice of the owner in `plan-k1`.
