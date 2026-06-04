# 230-vm-create-golden-linux

**Kind:** work

## Goal

Port **Linux golden creation** into `vm create-golden --platform linux` — a full
Rust port mirroring node `110`'s macOS port, reusing the `testanyware-vm`
russh/recovery/finalize layers. Delete `provisioner/scripts/vm-create-golden-
linux.sh` (587 lines) once ported + live-verified.

## Context

- **Standalone, loose timing** (`200`-Q2): unlike the *Windows* golden, the Linux
  golden **gates nothing in this wave** — the Linux verification harness (`190`,
  retired green) deliberately uses a **stock Ubuntu ARM64** image as its HUT, not
  this golden (ADR-0009 / 140-Q4: "no dependency on the deferred linux golden").
  So this leaf can land any time before grove-finish; it carries no downstream
  consumer. It is here because the root BRIEF "Done when" requires the full
  platform/distribution backlog complete.
- **macOS-host work, no cross binary** (`140` carried-in): the Linux golden is
  built on *this Mac* — tart clones the Ubuntu ARM64 image, the same way `110`
  built the macOS golden. Reuses `110`'s `golden`/`finalize`/recovery layers and
  the ADR-0007 `russh` `SshSession` (`sshd` is universal on Linux — the simplest
  provisioning channel, no agent needed).
- **No new ADR** (`200`-Q4): straight tart-based port under ADR-0007 (ssh-via-
  russh) + ADR-0008 (recovery-over-RFB/OCR). Raise one only if something
  genuinely surprises.

## Done when

- `vm create-golden --platform linux` produces a Linux golden on this Mac,
  **live-verified** by actually creating it ([[vm-costs]]: clone+start is cheap),
  mirroring how `110` was verified (golden produced; fresh clone reachable + agent
  healthy).
- `vm-create-golden-linux.sh` deleted; the `vm-create-golden` schema in
  `surface.rs` covers `--platform linux`; `release-build.sh` no longer bundles the
  deleted script.
- Root BRIEF distribution/golden checklist line updated.

## Notes

- The Windows golden is a **separate leaf** (`220/020`) inside the Windows arc —
  it *does* gate the Windows harness, so it lives on the critical path, not here.
- Acceptance gate: **CLI design contract**.
