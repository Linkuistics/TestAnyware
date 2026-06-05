# 215-docker-host-unification

**Kind:** research (fail-fast spike + findings doc; may raise an ADR and reshape
the root backlog if positive)

## HOISTED ahead of `220/050` (2026-06-05)

Originally numbered `240`, picked *after* the whole Windows arc. **Hoisted to
`215`** (picks before `220/050-windows-distribution`) by user decision once
`220/040` produced the **low-regret kill signal** this spike was waiting for:
EasyOCR is **uninstallable on aarch64-windows** (`opencv-python-headless` has no
`win_arm64` wheel anywhere; no in-guest MSVC to source-build it), so the
per-platform-native Windows host **cannot do OCR** without either an
out-of-scope toolchain build or a native `Windows.Media.Ocr` engine. Docker host
unification would **dissolve this structurally** — a Linux host binary in a
container runs the fully-wheeled EasyOCR stack regardless of host OS. That makes
the architectural question urgent *now*, with the Windows arc's evidence
(`040`: 2/3 bands green, OCR walled) in hand. **`220/050` (windows distribution)
is gated on this spike's outcome** — don't ship a native Windows binary that
docker may replace. The already-built Windows binary + `040` harness are
low-regret (the `030` source pass also serves a shipped Linux host; `040` reused
`190`'s machinery).

**Extra evidence for the spike:** the win-arm64 OCR wall is a concrete data
point on the *cost* of the per-platform-native-host path — weigh it in the
adopt/thin-shim/reject recommendation.

## Hypothesis

Replace the **per-platform host binaries** (the cross-compiled
`testanyware.exe` / future native hosts) with **one Linux binary run in a Docker
container**, giving the container enough access to its host machine — *maybe via
reverse SSH* — that it can do everything a native host binary does: drive the
hypervisor, reach a host-side framebuffer, manage goldens/clones, run the agent
channel. If viable, this would collapse the Tier-2 per-platform-host story
(Windows `040`/`050`, future macOS/Linux host variants) into a single
distributable artifact.

## Why this is a root-level spike (set 2026-06-05)

It questions the **per-platform-host-binary architecture itself**, not just the
Windows arc — so it sits at the root. Originally sequenced after the whole
`220-windows-arc`; now hoisted to `215` (above) so it runs after `040` (which
supplied the deciding evidence) but **before** `050` (which it may obviate). The
already-built Windows binary still got verified by `040`, and if this spike kills
the native path that work is low-regret: the `030` source pass also serves the
shipped Linux host binary, and `040`'s harness machinery is reused from `190`.

## The binding constraint this MUST engage first

**The host-side-framebuffer invariant (ADR-0010, [[CONTEXT.md]]).** The whole
`testanyware-rfb` stack *and* golden creation's pre-boot/recovery cycle
(ADR-0008) depend on a VNC/RFB endpoint served by **the hypervisor on the host**,
reachable **headless and before the guest OS boots** (boot screen, login,
recovery). Parallels Desktop was rejected (ADR-0010) precisely for offering only
a *guest-side* VNC. Any Docker proposal is judged against this gate **first** —
if the containerized binary cannot reach a host-side framebuffer for the VMs
under test, the idea is dead in its naïve form, exactly as Parallels was.

Concrete tensions the research must resolve, not gloss:

1. **A container can't run the host's hypervisor.** HVF (macOS) and WHPX/Hyper-V
   (Windows) are host-kernel facilities; a Linux container has no access to them.
   Running QEMU *inside* the container instead means the VM-under-test is no
   longer hosted by the platform's hypervisor — and on macOS/Windows **Docker
   Desktop is itself a Linux VM**, so that is **nested virt** (no host accel,
   often disabled), and the framebuffer it serves is the *container's*, not the
   *host's*. Does that even satisfy what we're testing?
2. **Reverse SSH needs a host-side listener.** To drive the host hypervisor from
   inside the container you need a host-side component. On **Windows there is no
   sshd** (the documented reason the in-VM agent channel exists). So "reverse SSH
   to the host" **re-introduces a platform-specific host component** — the very
   thing the hypothesis wants to remove.

## The honest reframe (likely the real research payoff)

The win is probably **not** "eliminate platform code" but **"shrink the
platform-specific host surface to a minimal shim"** — e.g. a tiny host-native
listener (sshd / a hypervisor-control stub) + the host's own hypervisor, with
**all real logic in one containerized Linux binary**. The research should
**quantify what irreducibly must stay native** per platform and judge whether
that shim is small enough to be worth the architectural change (build/ship/test
complexity of a container image + a host shim vs. N cross-compiled binaries).

## The spike (hands-on, on this Apple-Silicon Mac)

Mirror the `010`/`160` fail-fast pattern — cheap, concrete, decisive:

- Run the **Linux host binary in a container** on this Mac (Docker Desktop /
  `container`/colima — pick one, log which).
- Attempt to have it **drive a VM and reach a host-side framebuffer**, via the
  reverse-SSH (or other) bridge to the macOS host. Use the kept-built goldens
  ([[vm-costs]]: clone+boot is cheap).
- Determine empirically: can the container reach a *host-side* (pre-boot) RFB
  endpoint at all? What host-native piece was unavoidable? Measure the residual
  native surface.
- If macOS (sshd present) "works" via a shim, reason explicitly about the
  **Windows** case (no sshd) — the harder platform — before claiming generality.

## Prior art to scan

- How comparable tools do cross-platform VM control from containers (Docker
  Desktop's own host↔VM architecture; Lima/colima; tart's macOS-native model;
  vfkit/krunkit; QEMU-in-container nested-virt reports).
- Whether any expose a **host-side framebuffer** to a container, or whether all
  keep a host-native control plane.

## Done when

- A findings doc at `docs/research/240-docker-host-unification.md`: the spike
  result (can a container reach a host-side framebuffer for the VM-under-test?),
  the **quantified irreducible native surface per platform** (esp. Windows), and
  a clear **recommendation** — adopt / adopt-as-thin-shim / reject.
- If **reject:** record *why* (almost certainly promotes/leans on the
  host-side-framebuffer invariant, ADR-0010), so the question is not reopened
  blindly later.
- If **adopt / thin-shim:** raise an **ADR** for the new host-access model and
  **reshape the root backlog** (what happens to `040`/`050` and future
  per-platform host work) — likely spawning its own implementation grove rather
  than being built here.

## Notes

- Acceptance gate for any *resulting* host surface stays the **CLI design
  contract**; the harness pattern (ADR-0009) would need rethinking if the host
  is containerized.
- Honour [[minimal-images]] — a host shim must not bake test tooling into the
  host.
- Keep the spike a **fail-fast**: the host-side-framebuffer gate likely settles
  it in one cheap session; don't build a full containerized host before the gate
  is cleared.
