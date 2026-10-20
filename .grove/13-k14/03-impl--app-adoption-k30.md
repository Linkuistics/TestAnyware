# app-adoption-k30

## Goal

When the app starts, adopt each VM whose identity it can prove. Report the others, and signal nothing without proof.

## Context

- `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", the flowchart and the rules.
- `vm-process-identity-k15`, which records the identity.
- `qemu-legacy-stop-k31`, which stops an older QEMU guest through its control sockets. It lands before this leaf.
- The requirement scenarios of "VMs outlive the app".

## Done when

- A pure function takes a meta and the live process facts. It returns one outcome: adopt, stopped, doubt, or legacy.
- At start, the app reads each sidecar and applies the function. It serves its socket first, and it probes the agent and the VNC banner of each survivor later, with a deadline. A timeout means not answering, never dead.
- `vm stop` signals a process only on a full match of the pid, the start time and the boot session.
- When the identity does not match, `vm stop` signals nothing, and it also removes neither the clone, the session directory nor the sidecars. It fails with `VM_STOP_FAILED` and a remedy.
- A legacy VM gets streams. The app stops a tart legacy guest by name with `tart stop` and `tart delete`. It stops a legacy QEMU guest with the stop of `qemu-legacy-stop-k31`.
- `GET /v1/status` and the menu report each VM that the app did not adopt, with the reason.
- A test shows that a meta with a reused pid signals nothing and reports the VM as not adopted.
- A test shows that an older QEMU meta reaches the stop of `qemu-legacy-stop-k31` and never the signal path.

## Notes

- Before this leaf, the shared teardown still signals a QEMU guest with identity. This leaf closes that path.
