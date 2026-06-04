# Parallels Desktop as a third VM backend — feasibility findings

**Status:** research complete, 2026-06-04. Commissioned by grove leaf
`150-parallels-backend-research`.
**Audience:** the scope decision that closes leaf 150, and any future
evaluation of a non-tart/non-QEMU macOS-host backend (VMware Fusion, UTM,
cloud).
**Bottom line:** Parallels Desktop for Mac is **not viable** as a drop-in
backend, blocked by a single hard gate — it exposes **no host-side VNC/RFB
endpoint**. Lifecycle/golden mechanics map cleanly and Windows-on-ARM is a real
unique capability, but neither matters without a framebuffer the
`testanyware-rfb` stack can connect to.

> **Product-confusion warning (read first).** Most online "Parallels +
> `prlctl --vnc-port`" material refers to **Parallels Cloud Server /
> Virtuozzo** — a *different* Linux-host product that shares the `prlctl`
> binary name — **not** Parallels Desktop for Mac. This conflation is the
> single biggest trap in this research; every VNC claim below is tagged with
> which product it actually applies to.

---

## The one invariant that decides this

Every TestAnyware command needs a `host:port` (+ optional password) VNC/RFB
endpoint that the `testanyware-rfb` client drives — see the *Embedded viewer*
and RFB glossary entries. tart and QEMU both satisfy this **host-side**: the
*hypervisor* serves the framebuffer (tart via `tart run --vnc-experimental`
printing a `vnc://` URL; QEMU via a `-vnc` endpoint backed by its monitor
socket). Because the framebuffer is host-side, RFB works **before the guest OS
is up** — at the boot screen, the login window, and in macOS Recovery. That
pre-boot reach is not incidental: `vm create-golden` drives the framebuffer
over RFB+OCR through the SIP/TCC **recovery cycle** (ADR-0008) while no guest
agent exists yet.

A backend that can only surface a framebuffer from *inside* a booted guest
fails this invariant. Hold that criterion; it is what Parallels violates.

---

## Q2 — VNC / framebuffer access (the gate) — **FAILS**

**Parallels Desktop for Mac on Apple Silicon ships no built-in VNC server.**

- The `--vnc-mode {auto|manual|off}`, `--vnc-port`, `--vnc-passwd`,
  `--vnc-address` options are a **Virtuozzo / Parallels Cloud Server** feature
  (a Linux-host product), documented on the OpenVZ/Virtuozzo `prlctl(8)` man
  page — not Parallels Desktop.
  <https://static.openvz.org/vz-man/man8/prlctl.8.gz.html> ·
  <https://download.parallels.com/doc/pcs/html/Parallels_Cloud_Server_Users_Guide/36301.htm>
- The **official Parallels Desktop v19 Command-Line Reference** contains **no
  mention of VNC** — no `--vnc-mode`, `--vnc-port`, `--vnc-passwd`,
  `--vnc-address`. Explicit absence from the authoritative primary source.
  <https://download.parallels.com/desktop/v19/docs/en_US/Parallels%20Desktop%20Command-Line%20Reference.pdf>
- The **Parallels Desktop Developer's Guide** CLI page documents `prlctl` with
  no VNC capability.
  <https://docs.parallels.com/parallels-desktop-developers-guide/command-line-interface-utility>
- A user confirms (April 2022) that on Parallels Desktop "options like
  `--vnc-port` are no longer part of `prlctl`," with no staff rebuttal.
  <https://forum.parallels.com/threads/vnc-is-not-possible-for-vm-created-parallels.356811/>
- A 2023 feature request, "Integrated VNC server," asks Parallels to add
  exactly this — i.e. a user noting it does **not** exist. No staff
  commitment as of capture.
  <https://forum.parallels.com/threads/integrated-vnc-server.360455/>

**Consequence.** The only way to get an RFB endpoint out of a Parallels guest
is to **run a VNC server inside the guest OS** and reach it on the guest IP.
That is architecturally inferior on every axis that matters here:

- It is **guest-side**, so it cannot serve the boot screen, the login window,
  or macOS Recovery — killing the golden-creation proving ground (driver #2)
  outright (ADR-0008's recovery cycle has no booted OS to host a VNC server).
- It is **per-guest-OS work** (x11vnc/tigervnc on Linux, a Windows VNC service,
  Screen Sharing on macOS) — and in-VM guest software is a **separate
  workstream**, explicitly out of scope for this grove (`CONTEXT.md`,
  *In-VM agent*).
- It **overlaps** the in-VM agent's existing HTTP accessibility surface,
  reducing the marginal value to "screen capture / OCR after full boot" only.

**Edition gate (moot but noted).** `prlctl` itself is **Pro/Business-only**
(Standard has no CLI), so any CLI-driven backend already mandates a paid Pro
subscription — but VNC is absent in *every* edition, so this never becomes the
binding constraint.
<https://docs.parallels.com/parallels-desktop-developers-guide/command-line-interface-utility>
· <https://www.parallels.com/products/desktop/pro/>

---

## Q1 — Guest OS coverage on Apple Silicon — favorable, but moot given Q2

What Parallels adds over raw Virtualization.framework (tart):

- **Windows 11 ARM — the real, unique win.** Parallels is "the only
  virtualization solution Microsoft has authorized to run Windows 11 Pro and
  Enterprise on Apple silicon" (vTPM, Secure Boot, Windows Update, support
  eligibility), corroborated **independently by Microsoft**. Virtualization.
  framework / tart cannot run Windows at all.
  <https://www.parallels.com/products/desktop/microsoft-authorized-solution-windows-11-arm/>
  · <https://support.microsoft.com/en-us/windows/options-for-using-windows-11-with-mac-computers-with-apple-m1-m2-and-m3-chips-cd15fd62-9b34-4b78-b0bc-121baa3c568c>
- **Linux ARM64 — marginal.** Broader curated distro matrix (Ubuntu, Fedora,
  RHEL, Debian, Kali, CentOS Stream) plus Parallels Tools, but tart already
  runs the same ARM64 distros. The specific ARM Tools integration features
  (clipboard/resize) are not enumerated in primary docs.
  <https://kb.parallels.com/en/124223>
- **macOS guests — no real win.** Both Parallels and tart sit on
  Virtualization.framework for macOS guests, inheriting the same limits (≤2
  VMs, broken App Store/iCloud sign-in). Parallels adds little beyond its GUI.
  <https://kb.parallels.com/128867>
- **x86/x64 — additive but unusable for this purpose.** Parallels 20.2+ can
  emulate Intel guests, but "It is slow, really slow," 1 vCPU / 8 GB RAM, no
  USB/sound/nested-virt — an early preview unfit as a disposable-instance
  backend. <https://kb.parallels.com/130217>

So the payoff is essentially **Windows-on-ARM**. Real, but it cannot be
collected without a framebuffer (Q2), and the Tier-2 Windows-host story
already reaches Windows guests through QEMU.

---

## Q3 — Lifecycle CLI mapping — maps cleanly

Direct `prlctl` equivalents exist for every tart verb (all citations from the
Developer's Guide CLI reference,
<https://docs.parallels.com/parallels-desktop-developers-guide/command-line-interface-utility/manage-virtual-machines-from-cli/general-virtual-machine-management/>):

| tart | Parallels |
|---|---|
| `tart clone <base> <id>` | `prlctl clone <vm> --name <id> [--linked] [--template]` |
| `tart run <id>` | `prlctl start <vm>` |
| `tart stop <id>` | `prlctl stop <vm> [--kill]` |
| `tart delete <id>` | `prlctl delete <vm>` (vs `unregister` = forget-only) |
| `tart list --format json` | `prlctl list -a --json` (native JSON, `status` field) |
| — | `prlctl register <path>` / `unregister` (catalog in/out) |

Parallels even improves on tart in two spots: native `--json` output (no
URL-scraping) and a clean **`unregister` (forget) vs `delete` (destroy)** split
that tart lacks.

---

## Q4 — Headless start + IP discovery — weaker ergonomics, one load-bearing divergence

- **Headless is a mode, not a flag.** Parallels runs VMs windowless only when
  the Desktop GUI app is *not open* in the login session — there is **no
  documented per-command `--no-graphics`/`--headless` switch** (contrast tart).
  `prlctl start` with the GUI app open pops a window.
  <https://kb.parallels.com/en/123298>
- **No PID to track — the big one.** tart's `tart run` is a detached process
  *the tool owns* and SIGTERMs to stop. Parallels has no such handle:
  `prlctl start` is a fire-and-forget RPC to the **`prl_disp_service`
  dispatcher daemon**, which owns the VM lifetime (one `prl_vm_app` per VM,
  parented by the daemon). You **stop by name via `prlctl stop`**, not by
  signalling a tracked PID. The `VmMeta.pid` field (the detached-run pid) has
  no Parallels analogue.
  <https://kb.parallels.com/en/112764> ·
  <https://forum.parallels.com/threads/prl_disp_service.32801/>
- **IP discovery mirrors tart's staleness trap.** `prlctl list <id> --full
  --json` yields an `ip_configured` field (Parallels' own `vagrant-parallels`
  driver reads it; falls back to parsing
  `/Library/Preferences/Parallels/parallels_dhcp_leases`). The lease file goes
  **stale on reboot/clone** exactly like tart's cached `tart ip` — same
  mitigation applies: trust the IP only once `status == running`, re-read
  rather than cache.
  <https://github.com/Parallels/vagrant-parallels/blob/master/lib/vagrant-parallels/driver/base.rb>
  · <https://github.com/hashicorp/packer/issues/11431>
- **Auto-start-at-boot removed in v20+** — weakens any unattended/daemonized
  use on current versions. <https://kb.parallels.com/en/123298>
- **Headless-without-login is unverified.** No official statement that Parallels
  runs with **no GUI user logged in** (pure launchd/SSH); the Full-Disk-Access
  requirement and login-oriented guidance imply it expects a logged-in user.
  This would be the highest-risk unknown *if* the backend were otherwise
  viable. <https://kb.parallels.com/en/123298>

---

## Q5 — Golden model — maps cleanly

Parallels supplies all three primitives a clone-a-golden workflow needs:

- **Templates** (Pro-only): `prlctl clone --template` makes a template (a VM you
  can't run until converted back); GUI Convert-to-Template / Deploy. No
  documented `prlctl set --template on` toggle.
  <https://docs.parallels.com/landing/pdfm-ug/parallels-desktop-for-mac-26-users-guide/advanced-topics/working-with-virtual-machines/creating-and-using-virtual-machine-templates>
- **Linked clones**: `prlctl clone --linked` — fast, space-efficient CoW,
  conceptually equivalent to `tart clone`. Trade-off vs tart: a linked clone
  **depends on its parent** — deleting/altering the golden breaks live clones
  (tart's CoW clones are self-standing).
- **Register/unregister**: `prlctl register <path>` (intended for bundles
  "manually copied from another location", `--regenerate-src-uuid` to avoid
  collisions) supports build-once / clone-everywhere. A `.pvm` bundle is
  copyable host-to-host and not host-locked; the only binding is TPM-enabled
  (Windows 11) guests preferring the same Apple Account — irrelevant to a
  vTPM-free Linux golden.
  <https://docs.parallels.com/landing/pdfm-ug/parallels-desktop-for-mac-26-users-guide/advanced-topics/working-with-virtual-machines/transfering-a-virtual-machine-to-another-mac>

Clean on paper — but golden *creation* (not just cloning) is the surface Q2
kills, since provisioning a fresh golden drives the pre-boot/recovery
framebuffer that Parallels can't expose.

---

## Q6 — Licensing / availability constraints

- **CLI requires Pro or Business = subscription-only.** Standard (the only tier
  with a perpetual option, ~$220 one-time) has **no CLI**. Pro ~$120/yr, Business
  ~$150/yr, both subscription. There is **no perpetual license that includes
  `prlctl`**. <https://www.parallels.com/products/desktop/buy/>
- **VNC unlocked by no tier** (it does not exist — Q2).
- **CI/headless caveats**: auto-start-at-boot removed v20+; headless-without-
  login unverified (Q4). Both are moot given Q2, but would compound the cost.
- **EULA for programmatic control — unverified.** Parallels publishes the CLI,
  a Developer's Guide, and an official Vagrant provider (signalling
  programmatic control is intended), but the EULA's redistribution/activation
  clauses were not retrieved; read them before shipping automation **if** this
  is ever reopened.

---

## Q7 — Abstraction fit (codebase) — additive, would warrant an ADR *if* adopted

Studied against `cli-rs/crates/testanyware-vm/` HEAD:

- The backend seam is **hand-written dispatch, not trait polymorphism**.
  `VmTool` (`meta.rs`) is a **closed enum** (`Tart`, `Qemu`), and
  `lifecycle.rs` branches on it (or on `wants_tart()` + `#[cfg(target_os =
  "macos")]`) at **five sites**: `start`, `stop`, `delete`, `list`,
  `dry_run_validate_{start,delete}`. A third backend = a third arm at each
  site, plus a `VmTool::Parallels` variant.
- A `parallels.rs` would sit beside `tart.rs` as a third macOS-host arm
  (`#[cfg(target_os = "macos")]`), structurally **more tart-shaped than
  QEMU-shaped**: like tart it manages its own catalog/storage (so `VmMeta`
  carries no `clone_dir`), but **unlike tart it has no per-run PID** (Q4) — so
  the `VmMeta.pid` contract and the SIGTERM-the-run-process stop model would
  need a name-addressed `prlctl stop` alternative. That is the one place it
  does *not* drop in cleanly.
- **Three backends is the point where "extract a `trait VmBackend`" earns its
  place.** Two arms tolerate hand-written dispatch; a third tips the balance.
  Adopting Parallels should therefore be paired with an ADR choosing between
  (a) a third set of match arms, or (b) a backend-trait refactor — *and* a
  decision on the PID-vs-daemon stop-model divergence. Neither is forced now,
  because Q2 stops adoption before Q7 is reached.

---

## Recommendation

**Park as "investigated, not adopted."** Driver #2 (golden-creation proving
ground) is killed outright by Q2 — Parallels cannot expose the pre-boot/recovery
framebuffer ADR-0008 depends on. Driver #1 (meeting Parallels users where they
are) survives only via a guest-side VNC server, which is out-of-scope guest
software, redundant with the in-VM agent, and still can't create goldens — a
weak value proposition for the cost (Pro subscription, daemon stop-model
refactor, a new per-OS guest component).

The durable finding worth promoting is the **invariant**, not the rejection: a
TestAnyware VM backend must expose a **host-side framebuffer reachable headless
and pre-boot**. That criterion is what tart and QEMU satisfy, what Parallels
fails, and what any future backend candidate (VMware Fusion — which *does*
expose host-side VNC — UTM, cloud) should be judged against first. Captured in
ADR-0010.

## Findings adopted

- ADR-0010 (*VM backends must expose a host-side framebuffer; Parallels
  rejected*) cites this doc's Q2 as its rationale.
