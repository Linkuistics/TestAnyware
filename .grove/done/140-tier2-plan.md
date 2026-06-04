# 140-tier2-plan

**Kind:** planning

## Goal

Re-grill and decompose **Tier 2** — the Linux/Windows additive (beyond-parity)
work — shaped by Tier-1 outcomes. Kept lazy (grove constraint 4) until Tier 1
lands, because Tier 2's shape genuinely depends on Tier-1 results.

## Context

Tier 2 is **net-new capability** the Swift CLI never had (Swift = macOS-only),
but it is **self-verifiable**: TestAnyware runs up **Linux and Windows host-VMs**,
installs the **locally cross-compiled** (`zig cc`) host binary, and tests it
there. A host binary inside a guest drives a VM's agent/RFB endpoint over the
network, so the non-`vm-start` surface needs no nested virt. (This supersedes the
`070` "unverifiable in this env" framing.)

Inputs to grill against (all from Tier 1):
- `080` **cross-compile spike** outcome — feasible via `zig cc`, or fall back to
  build-on-target via VMs? This shapes the whole distribution leaf.
- `100` **`VideoEncoder` seam** — does the `ffmpeg-next` encoder drop in cleanly?
- `110` **golden port** shape — how much of the macOS golden orchestration
  generalizes to the linux/windows scripts (`vm-create-golden-{linux,windows}.sh`,
  587 + 514 lines).

Tier-2 items to decompose:
- **Linux-host support** (cross-platform pass): paths + `#[cfg]` facility wiring.
  **Lighter than Windows** — `process.rs`/`qemu_profile.rs` already carry the
  *Unix* path; the EasyOCR / ffmpeg-next / wgpu-on-Vulkan facilities are already
  anticipated (ADR-0002/0005/0006). Memory [[rust-port-conditional-facilities]].
- **Windows-host support** (cross-platform pass): process spawning, paths, the
  `#[cfg]` facility seams (`qemu_profile.rs`, `process.rs` stubs — "backlog task
  14"). The heavier of the two host passes.
- **Self-hosted verification harness**: run up Linux/Windows guests via
  TestAnyware, install the cross-compiled host binary, smoke-test the
  non-`vm-start` surface against a VM endpoint. Decide what a "host-under-test"
  VM is (vanilla guest vs reuse of the agent golden) and how the endpoint is
  provided. `vm start`/lifecycle-in-guest (nested virt) only if cheap.
- **`ffmpeg-next` encoders** for linux/windows `screen record` (ADR-0006 seam).
- **linux/windows `vm create-golden`** (full Rust port, per Q3).
- **linux/windows distribution** (Homebrew Linux + Windows zip), shaped by `080`.

## Decisions carried in (pre-grilling)

- **2026-06-03 (user):** linux + windows `vm create-golden` are a **full Rust
  port + live-verify**, mirroring node `110` — not a façade over the scripts.
  `vm-create-golden-linux.sh` / `vm-create-golden-windows.sh` are **deleted**
  once ported. Verified by **actually creating each golden on the macOS host**
  (cheap — [[vm-costs]]), as `110` was. So the grilling for this item starts from
  *how* to port (reuse of `110`'s `golden`/`finalize`/recovery layers, the
  QEMU+swtpm Windows path, the ssh-vs-other provisioning channel per guest), not
  *whether* to port. Sequencing held **after Tier 1** (this leaf stays after
  `120`/`130`).
- **Host-side vs host-support — keep these two axes distinct.** Golden
  *production* for linux/windows guests is **macOS-host work** (tart clones the
  Ubuntu image; QEMU+swtpm drives the Windows 11 ARM64 installer) — it runs on
  the Mac, like the `110` macOS golden. That is **separate** from **Linux-host /
  Windows-host support** (running the `testanyware` CLI *on* a Linux/Windows
  host). The two Tier-2-items bullets below ("Linux/Windows-host support" vs
  "linux/windows `vm create-golden`") are therefore genuinely different work; the
  golden port needs **no** cross-compiled host binary and **no** non-macOS host —
  it can be built and verified entirely on this Mac today.

## Decisions (running log — 140 grilling, 2026-06-04)

- **Q1 — first-wave sequence: `ffmpeg-spike → ffmpeg-impl → Linux-host → harness`.**
  Front-load a small `ffmpeg-next` cross-build spike (fail-fast on the one
  residual cross-compile risk the `080` spike flagged — `ffmpeg-next` links
  system `libav*` at *link time* via pkg-config, unlike `dlopen`-ed `wgpu`),
  then the `ffmpeg-next` `VideoEncoder` impl, then the Linux-host source pass
  (lighter than Windows — `process.rs`/`qemu_profile.rs` already carry the Unix
  path), then the self-hosted verification harness, **first exercised on Linux**.
  Windows-host pass, linux/win distribution, and linux/win `vm create-golden`
  are **deferred** (kept as the root-brief checklist, materialized when their
  turn comes — mirroring how `070` deferred its later wave to `140`). Linux is
  the proving ground for the whole cross-platform pattern before the heavier
  Windows pass. Rejected: materialize-all-6-now (Windows/harness briefs would be
  guesses until the Linux pass teaches their shape — tension with constraint 4);
  golden-first (independent macOS-host work, but doesn't advance the host story).

- **Q2 — target-triple matrix: four targets, both arches per OS.**
  `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `x86_64-pc-windows-*`, **`aarch64-pc-windows-*`** (user: "we should definitely
  build win/aarch64"). The architecture realization driving this: every guest
  this Apple-Silicon Mac boots *natively* is **ARM64** (tart Ubuntu ARM64; QEMU
  Windows 11 **ARM64**), and the `080` spike built only **x86_64** — an x86_64
  binary cannot run on an ARM64 guest. So the **aarch64** targets are the ones
  the self-hosted harness can natively in-guest verify; promoting
  `aarch64-windows` to first-class makes it the *verifiable* Windows build (the
  only Windows guest we can boot here). **x86_64** builds stay **build/link-
  verified** (`080`) with the runtime gap **logged** (no-silent-caps), unless a
  real/emulated x86_64 host is cheap. **Consequence — `160` spike scope widens:**
  the `080` spike proved only x86_64-linux (full) + x86_64-windows (toolchain);
  the new aarch64-linux, aarch64-windows, and x86_64-windows *full link* paths
  are **not yet proven**, and the Windows targets want the cross-friendly
  **`-gnu`/`-gnullvm`** variants (msvc can't cross from a Mac). So `160` is a
  *complete-the-cross-build-matrix* spike (all four triples) **with `ffmpeg-next`
  folded in**, not an ffmpeg-only spike.

- **Q3 — harness endpoint: real tart macOS golden via a host port-forward.**
  The in-guest cross CLI drives `host-gateway:PORT`; the macOS host forwards
  (`socat` / `ssh -L`) to a cheap kept-built tart macOS golden's agent (`:8648`)
  + VNC. Picked for the reliable network edge (guest→host-gateway always routes
  on NAT; guest→other-guest does not without bridging) and real agent/RFB wire
  behavior, reusing the cheap golden ([[vm-costs]]) with no stub to maintain.
  Matches the root brief's "drive a VM's agent/RFB endpoint over the network"
  intent while solving reachability via the host forward. Rejected: stub-on-host
  (deterministic but tests a fiction unless perfectly faithful — and the CLI's
  RFB/agent *client* code is already macOS-tested by the live-vm-gate, so the new
  variable is just "does the cross binary run + speak the wire on this OS/arch",
  which a real endpoint answers honestly); guest-loopback (needs the out-of-scope
  in-VM agent inside the HUT). **Endpoint-free surface** (capabilities, schema,
  llm-instructions, doctor, `--help`, dry-runs) needs no target and is the
  cheapest highest-value check — it proves the cross binary actually *executes*
  on the target (dynamic loader, glibc floor, OCR-engine init).

- **Q4 — HUT VM + provisioning channel: asymmetric, split by what channel the
  guest OS offers.** The host-under-test runs the cross CLI; it is the *host*,
  not the target.
  - **Linux HUT = stock tart Ubuntu ARM64**, provisioned via **russh/ssh** (reuse
    the 110 SSH layer) — `sshd` is universal on Linux, so no agent and **no
    dependency on the deferred linux golden** (D). Fully self-contained.
  - **Windows HUT = the Windows agent-golden**, provisioned via the **in-VM
    agent's `file upload` + `exec` HTTP surface** — **Windows ships no SSH
    server** (user correction), so the agent is the only in-guest control
    channel. This **couples the Windows harness to (i) the Windows golden and
    (ii) a working Windows in-VM agent** — both heavier, both partly in the
    separate agents workstream. Reinforces **Linux-first**: the Linux harness is
    self-contained; the Windows harness waits on the Windows golden + agent.

  Rejected: reuse the agent-golden for *both* (needlessly couples the cheap Linux
  path to deferred golden work); a dedicated HUT image pipeline (overkill).
  **Flag for the Windows harness leaf:** confirm the Windows agent exposes
  `file`/`exec` and is installable, else the Windows harness is blocked on the
  agents workstream.

- **Q5 — surface split: three bands.**
  - **Endpoint-free smoke** (in-guest, no target): `capabilities`, `schema`,
    `llm-instructions`, `doctor`, `--help`, dry-runs of mutating commands.
  - **Endpoint-driven smoke** (in-guest, against the forwarded golden): `agent`
    HTTP actions, all `input *`, `screen capture`/`size`/`find-text` (OCR),
    `screen record`→mp4 (ffmpeg-next). Proves the high-risk cross facilities
    (RFB client, OCR engine, libav) actually load + run on the target.
  - **Build/compile-only** (not exercised in-guest): `vm start/stop/list/delete`
    + `vm create-golden` (need nested virt / are host-orchestration). Brief's
    "lifecycle-in-guest only if cheap" → default out.
  - **Arch coverage:** **aarch64** builds get full in-guest smoke (native guests
    on this Mac); **x86_64** builds are **build/link-verified only** (no native
    x86_64 guest here) with the runtime gap **logged** (no-silent-caps).

- **Distribution interleave (root-brief open Q): per-OS, trailing that OS's
  host-pass + harness.** Linux distribution ships only after the Linux-host pass
  + Linux harness prove the binary runs; Windows distribution after the
  Windows-host pass + Windows harness. Never ship a binary the harness has not
  run green. No pushback on shipping the proven-*building* Linux binary earlier,
  so the trailing rule stands.

## Done when

- Tier-2 leaves/nodes materialized with clear briefs (via
  `grove-llm leaf-add`/`leaf-insert`).
- Sequencing decided — Linux-host (lighter) likely before Windows-host, and
  **whether the host passes and distribution interleave** (the open question the
  root brief flagged); the self-hosted verification harness gates "done" for both.
- ADRs raised only where hard-to-reverse/surprising.
- The self-hosted verification approach concretized (host-under-test VM shape,
  endpoint provisioning, which surface is smoke-tested vs compile-only).

## Notes

- Grill one question at a time, recommended answer per question (`grilling.md`,
  `driving.md`). Best in a **fresh session**.
- Acceptance gate for resulting work leaves stays the **CLI design contract**.
