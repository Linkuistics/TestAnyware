# 020-vm-create-golden-windows

**Kind:** work

## Goal

Full **Rust port** of Windows golden creation into `vm create-golden --platform
windows`, mirroring node `110`'s macOS port. Reuses the `testanyware-vm`
recovery/finalize layers and the QEMU+swtpm path; provisions over the in-VM
agent (no SSH on Windows). Delete `provisioner/scripts/vm-create-golden-
windows.sh` (514 lines) once ported + live-verified.

## Context

- **This golden IS the harness HUT** (`200`-Q2, ADR-0009) — it gates `040`. That
  is why the Windows golden lives on the critical path inside this arc, while the
  *Linux* golden (`230`) is standalone/loose.
- **Provisioning differs from macOS/Linux** (no sshd): the Windows golden is built
  via **autounattend unattended-install** (`provisioner/autounattend/` — VirtIO
  drivers, .NET 9, the `TestAnywareAgent` logon task) and controlled through the
  **agent's `/exec`/`/upload`** surface, *not* the ADR-0007 `russh` channel that
  macOS/Linux use. QEMU+swtpm (`testanyware-vm`'s `qemu.rs`/`qemu_profile.rs`),
  not tart.
- **No new ADR** (`200`-Q4): this approach is **already established** in the
  existing `vm-create-golden-windows.sh` + `provisioner/autounattend/` — the port
  *documents* it, it does not decide it. **Capture the autounattend/agent
  provisioning model inline in CONTEXT.md** (a glossary entry) as part of this
  leaf; raise an ADR only if the recovery/provisioning model proves a genuinely
  new trade-off.
- Builds on `010`'s GREEN disposition — `010` already proved the golden is
  creatable + agent green via the shell script; this leaf turns that into the
  Rust subcommand.

## Done when

- `vm create-golden --platform windows` produces a Win11 ARM64 golden on this Mac,
  **live-verified** by actually creating it ([[vm-costs]]) — fresh clone boots,
  agent healthy, `/exec` round-trips (the HUT capabilities `040` needs).
- `vm-create-golden-windows.sh` deleted; `release-build.sh` no longer bundles it;
  the `vm-create-golden` schema in `surface.rs` covers `--platform windows`.
- The autounattend/agent provisioning model is documented in CONTEXT.md.

## Notes

- Reuse: `110`'s `golden`/`finalize`/recovery scaffolding + the QEMU+swtpm machinery
  the Windows `vm start` path already exercises. The novel arm is the
  agent-channel provisioning (vs ssh).
- Acceptance gate: **CLI design contract**.
