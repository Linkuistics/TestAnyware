# 010-windows-readiness-spike

**Kind:** work

## Goal

**Fail-fast feasibility check, zero Rust.** Confirm the hardest external
dependency of the whole Windows arc is real: that a Windows golden is **creatable
today** and the in-VM agent **runs green**. Use the *existing*
`provisioner/scripts/vm-create-golden-windows.sh` to build a Win11 ARM64 golden,
boot a clone, and confirm the agent reaches health and the `/exec` + `/upload` +
`/download` endpoints respond.

## Context

- **Gates the whole arc** (`200`-Q3). The Windows harness (`040`) uses the Windows
  agent-golden as its **HUT**, provisioned over the agent's `/upload` + `/exec`
  (Windows ships no sshd — the agent is the only in-guest control channel,
  ADR-0009). If that channel is broken, the arc is blocked on the *separate,
  out-of-scope* agents workstream — better to learn it now for ~1 cheap session
  ([[vm-costs]]) than after porting golden + host-pass. Mirrors the `160`
  fail-fast spike.
- **What's known going in:** the agent's control surface **exists in code** —
  `agents/windows/SystemEndpoints.cs` exposes `/exec`, `/upload`, `/download`; the
  agent cross-builds from this Mac (`dotnet -r win-arm64 --no-self-contained`) and
  installs via `provisioner/autounattend/` + a `TestAnywareAgent` logon task
  (`agents/windows/README.md`). Unproven is whether it boots green in a fresh
  golden. **VirtIO networking drivers** (installed by the autounattend XML) are
  load-bearing — without them the agent is unreachable.
- **Don't modify the image beyond essentials** ([[minimal-images]]); this spike
  *uses* the existing provisioner, it does not add test tooling.

## Done when

- A Win11 ARM64 golden is created via the existing script; a fresh clone boots and
  the agent answers `agent health` + a round-trip `/upload`→`/exec`→`/download`
  (the `ProvisionChannel` operations `040` will need).
- **Disposition recorded** in this leaf's commit / the `220` BRIEF: GREEN (arc
  proceeds to `020`) or BLOCKED (which agent/golden gap, filed against the agents
  workstream via `grove-llm inbox-add` to the relevant grove). No-silent-caps.

## Notes

- Pure feasibility — no Rust, no port. The Rust golden port is `020`.
- If the existing script itself is stale/broken, that's a finding too — capture it
  for `020` (which replaces the script).
