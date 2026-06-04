# 10. VM backends must expose a host-side framebuffer; Parallels Desktop is rejected on that gate

Date: 2026-06-04

## Status

Accepted

## Context

TestAnyware supports two VM backends — **tart** (Apple Virtualization.framework,
macOS host) and **QEMU** — dispatched as a closed `VmTool` enum across five sites
in `cli-rs/crates/testanyware-vm/src/lifecycle.rs`. Leaf
`150-parallels-backend-research` investigated **Parallels Desktop for Mac** as a
third backend, motivated by (1) existing users who already run Parallels and (2)
a second, non-tart macOS hypervisor to exercise the golden-creation machinery
and prove the backend abstraction isn't accidentally tart-shaped.

Every command in the surface needs a `host:port` (+ optional password) VNC/RFB
endpoint that the `testanyware-rfb` client drives. tart and QEMU both supply
this **from the host side**: the *hypervisor* serves the framebuffer (tart via
`tart run --vnc-experimental`'s `vnc://` URL; QEMU via a `-vnc` endpoint backed
by its monitor socket). Because the framebuffer is host-side, RFB works **before
the guest OS is up** — at the boot screen, the login window, and in macOS
Recovery. That pre-boot reach is load-bearing, not incidental:
`vm create-golden` drives the framebuffer over RFB+OCR through the SIP/TCC
**recovery cycle** (ADR-0008) while no guest agent yet exists.

The research (`docs/research/parallels-backend-feasibility.md`) found that
**Parallels Desktop for Mac exposes no host-side VNC server.** The
`--vnc-mode`/`--vnc-port`/`--vnc-passwd`/`--vnc-address` options widely cited
online belong to **Parallels Cloud Server / Virtuozzo** — a different Linux-host
product sharing the `prlctl` binary name. The official Parallels Desktop v19
Command-Line Reference contains no VNC mention; a 2023 "Integrated VNC server"
feature request sits unanswered; a user confirms `--vnc-port` is "no longer part
of `prlctl`." The only RFB path out of a Parallels guest is a **guest-side VNC
server**, which (a) cannot serve the pre-boot/recovery framebuffer ADR-0008
needs, (b) is per-guest-OS software in a workstream explicitly out of scope for
this grove, and (c) overlaps the in-VM agent's existing HTTP surface.

Parallels' other mechanics are favorable but moot without a framebuffer:
lifecycle verbs map cleanly (`prlctl clone/start/stop/delete`, native
`prlctl list --json`), the golden model maps (templates, `--linked` clones,
`register`/`unregister`), and Windows-on-ARM is a genuine Microsoft-authorized
capability tart cannot match. One lifecycle divergence was also noted: Parallels
has **no per-run PID** to track — `prlctl start` is a fire-and-forget RPC to the
`prl_disp_service` dispatcher daemon — so its stop model is name-addressed
(`prlctl stop`), not the SIGTERM-the-tracked-pid model tart and QEMU use.

## Decision

**A TestAnyware VM backend must expose a host-side framebuffer: a VNC/RFB
endpoint, served by the hypervisor, reachable headless and before the guest OS
boots (boot screen, login window, recovery).** This is the first gate any
backend candidate is evaluated against, ahead of lifecycle-CLI fit, golden
model, or guest coverage.

**Parallels Desktop for Mac is rejected** as a backend because it fails this
gate. Leaf 150 is retired "investigated, not adopted"; no `parallels.rs` arm and
no `VmTool::Parallels` variant are added.

## Consequences

- The host-side-framebuffer requirement is now an explicit, citable criterion.
  Future candidates (VMware Fusion — which *does* expose host-side VNC — UTM,
  cloud hypervisors) are judged on it first; a candidate that only offers a
  guest-side framebuffer is rejected without further investigation.
- The backend abstraction stays two-armed (`Tart`, `Qemu`). The "extract a
  `trait VmBackend`" refactor that a genuine third backend would have justified
  is **not** undertaken now; the hand-written five-site dispatch remains
  adequate.
- Driver #2 (a second golden-creation surface) is closed: no non-tart macOS
  hypervisor that meets the gate is currently in hand, so the tart golden path
  remains the sole macOS-guest implementation. The abstraction-proving value
  rolls into the Tier-2 Linux/Windows host work instead.
- Driver #1 (meet Parallels users) is not served. Should it be reopened, the
  only path is a guest-side VNC server scoped to *running* pre-provisioned VMs
  (never golden creation), accepting out-of-scope guest software and a Pro
  subscription — a deliberately deferred, low-priority option.
- The research doc is retained under `docs/research/` as the evidence base; this
  ADR cites its Q2 section as the rationale.
