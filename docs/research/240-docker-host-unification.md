# Docker host unification — feasibility findings

**Status:** research complete (fail-fast spike), 2026-06-07. Commissioned by grove
leaf `215-docker-host-unification` (originally numbered `240` — the doc keeps the
`240` path the brief specified; the leaf was hoisted to `215` to pick after the
Windows harness `220/040` but before Windows distribution `220/050`).
**Audience:** the architecture decision on whether to replace the per-platform
host binaries with one containerized Linux binary, and the gating of `220/050`
(Windows distribution) that waits on this outcome.
**Bottom line:** the naïve hypothesis — **one Linux host binary in a container,
replacing the per-platform host binaries** — is **rejected**. It fails the
host-side-framebuffer gate (ADR-0010) on exactly the two platforms it would need
to help (macOS, Windows), helps only on Linux (already runtime-GREEN, the easy
platform), and the irreducible native surface it leaves behind (a host-served
framebuffer + a host-native hypervisor + a host-side control listener — *and on
Windows that listener cannot be sshd*) is not smaller than the N cross-compiled
binaries it would replace. **However**, the spike surfaced a genuinely sound,
*much narrower* structural win that the broad framing accidentally contained:
**containerizing only the OCR engine** (a Linux EasyOCR daemon) dissolves the
win-arm64 OCR wall without touching the framebuffer/VM-control surface at all,
because OCR is host-side compute *downstream* of framebuffer capture and has no
hypervisor dependency. That carve-out is carried forward as the resolution path
for the deferred Windows OCR band (vs. the ADR-0002 native `Windows.Media.Ocr`
option) — it is **not** docker-host-unification.

---

## The one invariant that decides the broad hypothesis

**The host-side-framebuffer invariant (ADR-0010, [[CONTEXT.md]]).** Every
TestAnyware command needs a `host:port` VNC/RFB endpoint served by the
**hypervisor on the host**, reachable headless and **before the guest OS boots**
— boot screen, login window, recovery. `vm create-golden` drives that
framebuffer over RFB+OCR through the SIP/TCC **recovery cycle** (ADR-0008) while
no guest agent yet exists. tart and QEMU both satisfy it host-side; Parallels
was rejected (ADR-0010) precisely for offering only a *guest-side* framebuffer.

A containerized host binary is judged against this gate **first**. The two
tensions the brief refused to let me gloss, resolved below, both bear on it.

---

## Hands-on probe (this Apple-Silicon Mac)

Per the brief's fail-fast directive — *the host-side-framebuffer gate likely
settles it in one cheap session; don't build a full containerized host before
the gate is cleared* — I did **not** build the reverse-SSH bridge. The gate is
settled by the runtime facts a container exposes, which are decisive on their
own. Runtime logged:

| Probe | Result | Why it matters |
|---|---|---|
| Container runtime | **Docker Desktop 28.1.1**, server `linux/arm64` | the one available here (no colima/lima/Apple `container`) |
| Container kernel | `6.10.14-linuxkit`, `OSType=linux` | the "Linux host" a container sees on macOS **is Docker Desktop's own LinuxKit VM**, not macOS — any framebuffer it serves is the *container-VM's*, not the *host's* |
| `/dev/kvm` in container | **absent** (`No such file or directory`) | no hypervisor passthrough — QEMU-in-container would be unaccelerated TCG |
| nested-virt CPU flags (`vmx/svm/hvf`) in container | **none exposed** | confirms no nested virtualization is available to the container |
| Host chip | **Apple M1 Pro**, macOS 26.5.1 | the M1 family has **no nested-virt support at all** (Apple added VF nested virt only on **M3+**, macOS 15+) — so on this hardware the absence above is structural, not a config toggle |
| Host hypervisor present | `tart 2.32.1` (Virtualization.framework) | the real host-side framebuffer source — runs on macOS, **uncallable from a Linux container** |

---

## Tension 1 — "a container can't run the host's hypervisor"

Confirmed and decisive.

- **macOS / Windows hypervisors are host-kernel facilities.** Virtualization.
  framework / HVF (macOS) and WHPX / Hyper-V (Windows) live in the host kernel. A
  Linux container has no syscall path to them. `tart` (a VF consumer) therefore
  **cannot run inside the container**. Empirically: no `/dev/kvm`, no virt flags.
- **Running QEMU *inside* the container instead is nested virt** — and on
  macOS/Windows Docker Desktop is *itself* a Linux VM, so a container's QEMU runs
  two levels deep. On this M1 Pro that is **TCG software emulation** (no
  acceleration), unusably slow for booting real OS guests. Even on M3+/macOS-15+
  where VF nested virt exists, Docker Desktop does not surface `/dev/kvm` to
  containers today (confirmed absent here), so it would still fall back to TCG.
- **The framebuffer a container's QEMU serves is the *container's*, not the
  *host's*.** It would satisfy the letter of "an RFB endpoint" but not the
  invariant's intent: it is not the platform's native hypervisor hosting the
  guest-under-test.
- **The macOS-guest case is a hard wall, independent of speed.** The primary
  guest-under-test is a **tart macOS golden**. macOS guests on Apple Silicon may
  *only* be virtualized through Virtualization.framework (Apple licensing + no
  QEMU VF-guest backend). So a container's QEMU **can never host the macOS
  golden** — not slowly, not at all. For macOS-guest testing the container is
  structurally incapable of hosting the guest itself; it could only ever *drive a
  host-side tart* remotely.

**Gate result, macOS & Windows: FAIL in the naïve form.** The container cannot be
the hypervisor and cannot serve the host-side framebuffer. The only path is the
container driving the host's hypervisor + consuming a host-served framebuffer —
i.e. a host-native control plane survives. That is the thin-shim reframe, Tension 2.

**Linux is the exception — and it is the platform we don't need help on.** On a
*native* Linux host the container shares the host kernel; QEMU-in-container with
`--device /dev/kvm` gets real KVM acceleration and serves a host-reachable,
pre-boot `-vnc` endpoint. On Linux the invariant is genuinely satisfiable in a
container. But the Linux host CLI **already runs natively and is runtime-GREEN**
across all three bands (`180`/`190`). Containerizing Linux solves a solved
problem; it adds an image to build/ship for zero capability gain.

---

## Tension 2 — "reverse SSH re-introduces a platform-specific host component"

Confirmed. To drive the host hypervisor from inside the container you need a
host-side listener.

- **macOS:** the host has sshd available as opt-in **Remote Login**. So a thin
  shim is *possible* — container → `ssh`/port-forward → host `tart` + host-served
  framebuffer. But this changes nothing about cost on macOS: the macOS host CLI
  **already runs natively** (Tier-1, `cli/` already deleted at parity). Adding a
  container + a Remote-Login dependency + a forwarding shim to reach a hypervisor
  the native binary already drives in-process is pure overhead.
- **Windows:** **Windows ships no sshd** (the documented reason the in-VM agent
  channel exists — see [[CONTEXT.md]] *Autounattend provisioning*). So "reverse
  SSH to the host" is unavailable; the shim must be a **custom host-native
  listener** (a hypervisor-control stub). That is net-new, platform-specific,
  *host-side* code — **exactly the thing the hypothesis set out to delete.** The
  Windows host CLI already runs **2/3 bands GREEN** (`220/040`) as a cross-
  compiled native binary; replacing it with (container image + custom Windows host
  listener + forwarding) is strictly more surface, not less.

---

## Quantified irreducible native surface per platform

What docker-host-unification **cannot** remove, by platform:

| Platform | Irreducible host-native surface under the container model | Verdict |
|---|---|---|
| **macOS** | The hypervisor (VF/tart) **and** the host-served framebuffer **and** a host listener (Remote Login/sshd, opt-in) for the container to drive it. macOS guest cannot be a container-QEMU guest at all. | No code removed; native binary already exists and is simpler. |
| **Windows** | The hypervisor (WHPX or a host QEMU) **and** the host-served framebuffer **and** a **custom host listener** (no sshd → new platform-specific host component). | *Adds* native host surface; worse than the existing 2/3-GREEN cross binary. |
| **Linux** | Nearly nothing (KVM is the shared-kernel hypervisor; container can serve `-vnc`). | Genuinely containerizable — but already solved natively; zero gain. |

The surface does not shrink to "a tiny shim." On the platform that matters most
for the motivating problem (Windows) it *grows*. The honest reframe the brief
anticipated — *shrink the platform-specific host surface to a minimal shim* —
does not pay out: there is no version where the container removes more native
code than it adds, once the framebuffer-must-be-host-served invariant is honored.

---

## Prior-art scan

- **Docker Desktop's own architecture** (confirmed locally): containers run in a
  **host-native** LinuxKit VM that *Apple VF runs on the host*. Docker itself
  keeps a host-native hypervisor/control plane; it does not expose that VM's — let
  alone the host's — framebuffer to containers. The pattern is "host-native VM
  manager, containers as guests," the opposite of "container manages host VMs."
- **Lima / colima:** run guest VMs via VF or QEMU **on the host**; the control
  plane and any display are host-native. No container-side host-VM control.
- **tart:** explicitly macOS-native (VF); its `--vnc-experimental` framebuffer is
  host-served — the model TestAnyware already depends on.
- **vfkit / krunkit:** host-native VF/libkrun front-ends; again host-side.
- **QEMU-in-container nested-virt reports:** viable only with `/dev/kvm`
  passthrough on a **native Linux host**; on macOS/Windows Docker it is nested and
  unaccelerated (matches the empty `/dev/kvm` probe here).

No surveyed tool exposes a **host-side** framebuffer to a container, or moves
host-VM *control* into a container. Every one keeps a host-native control plane —
the same conclusion the gate forces.

---

## Recommendation

**REJECT** docker-host-unification (the whole-host-binary-in-a-container
hypothesis). It fails the host-side-framebuffer gate (ADR-0010) on macOS and
Windows, is redundant on Linux, and leaves an irreducible native surface that on
Windows is *larger* than the status quo. Cross-compilation via `cargo-zigbuild`
is already proven; the Windows host CLI already runs 2/3 bands GREEN. The
container path trades a working, simpler N-binary distribution for a
container-image-plus-host-shim architecture that removes no native code.

This **does not reopen** the host-side-framebuffer invariant — it leans on it, as
ADR-0010 intended, so the broad question is not relitigated blindly later.

### The narrow win to carry forward (NOT docker-host-unification)

The spike's true motivator was the **Windows OCR wall**: EasyOCR is uninstallable
on win-arm64 (`opencv-python-headless` has no `win_arm64` wheel). The broad
hypothesis over-reached by trying to containerize the *whole host* to fix it. The
correct, minimal insight:

> **OCR is host-side compute on a captured PNG, *downstream* of framebuffer
> capture, with no hypervisor dependency.** The host CLI's RFB client (pure Rust,
> already build/link-verified and `screen capture`-GREEN on win-arm64 in `220/040`)
> fetches the frame; OCR then runs on that image. That step needs no VF/WHPX/KVM —
> so it can run in a **Linux container on any host OS** (Docker Desktop runs Linux
> containers natively; **no nested virt**, because there is no VM here at all).

So the **OCR daemon** (`OcrChildBridge`, already a separate Python child process —
[[CONTEXT.md]] *OCR daemon*) can be hosted as a **Linux EasyOCR container**, and
the native Windows host CLI ships captured PNGs to it over a local socket/HTTP.
The fully-wheeled Linux EasyOCR stack then serves Windows OCR regardless of
host arch, **without containerizing the framebuffer or VM-control surface**.

This is a clean candidate for the **ADR-0002 per-platform `OcrEngine` seam** — a
third engine variant (`easyocr_container`) alongside the daemon and the native
options — to be weighed against the other deferred option, a native
**`Windows.Media.Ocr`** engine. It is **gate-irrelevant** (touches nothing
ADR-0010 governs) and far cheaper than docker-host-unification. It belongs to the
Windows OCR band, not to a host-architecture change.

---

## Backlog impact

- **`220/050` (Windows distribution) is UNGATED** by this outcome — ship the
  native cross-compiled Windows host binary (the model `220/040` verified 2/3
  bands on). Docker will not replace it.
- **Deferred Windows OCR band** gets a concrete resolution path: evaluate
  **containerized Linux EasyOCR** vs **native `Windows.Media.Ocr`** at the
  ADR-0002 seam. A leaf is added for that decision.
- **No ADR raised for a new host-access model** (the adopt/thin-shim branch did
  not fire). ADR-0010 already records the invariant this rejection rests on; this
  findings doc is the durable rationale, cited from the root brief.
