# app-adoption-k30

## Goal

When the app starts, adopt each VM whose identity it can prove. Report the others, and signal nothing without proof.

## Context

- `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", the flowchart and the rules.
- `vm-process-identity-k15`, which records the identity.
- The requirement scenarios of "VMs outlive the app".

## Done when

- A pure function takes a meta and the live process facts. It returns one outcome: adopt, stopped, doubt, or legacy.
- At start, the app reads each sidecar and applies the function. It serves its socket first, and it probes the agent and the VNC banner of each survivor later, with a deadline. A timeout means not answering, never dead.
- `vm stop` signals a process only on a full match of the pid, the start time and the boot session.
- A legacy VM gets streams. The app stops a tart legacy guest by name with `tart stop` and `tart delete`.
- `GET /v1/status` and the menu report each VM that the app did not adopt, with the reason.
- A test shows that a meta with a reused pid signals nothing and reports the VM as not adopted.

## Notes

- The stop of a legacy QEMU guest is the next leaf.
