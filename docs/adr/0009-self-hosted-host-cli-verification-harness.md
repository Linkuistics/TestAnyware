# 9. The cross-compiled host CLI is verified by running it inside native-arch guests driving a real golden through a host port-forward

Date: 2026-06-04

## Status

Accepted

## Context

Tier 2 of the `rust-cli-port` grove ships the `testanyware` host CLI for Linux
and Windows (the Swift CLI was macOS-only). The CLI is cross-compiled locally on
an arm64 Mac via `cargo-zigbuild` (the `080` spike proved the toolchain). A
green cross-*build* is not proof the binary *runs* on the target: the dynamic
loader, glibc floor, the per-platform OCR engine (EasyOCR via the OCR daemon),
the `ffmpeg-next` libav link, and the `dlopen`-ed Vulkan/wgpu stack can all fail
only at runtime, on the target OS/arch. TestAnyware's own VM machinery is the
natural way to get a real target host to run the binary on — "test the host CLI
with the product."

Two facts shape the design:

1. **Architecture mismatch.** Every guest this Apple-Silicon Mac boots
   *natively* is **ARM64** (tart Ubuntu ARM64; `qemu-system-aarch64` + swtpm
   Windows 11 ARM64). The `080` spike built **x86_64** binaries. An x86_64
   ELF/PE cannot run on an ARM64 guest, so only **aarch64** builds are natively
   in-guest verifiable here. (This is why the distribution matrix promotes
   `aarch64-pc-windows-*` to first-class — it is the *only* Windows build the
   harness can run; x86_64-Windows runs only on real x86_64 Windows.)

2. **No reliable guest→guest network.** On NAT'd virtualization a guest's
   default gateway *is* the host, so anything bound on the host is reachable from
   the guest; guest→*other-guest* routing (a QEMU Linux guest reaching a tart
   macOS VM) is not guaranteed without bridged networking.

The endpoint-requiring CLI surface (agent HTTP, input/screen over RFB, OCR)
needs a live agent + VNC endpoint to drive. The in-VM agent and the goldens are
a *separate, out-of-scope* workstream — unchanged by Tier 2 — so the variable
under test is the **cross-compiled client binary**, not the endpoint.

## Decision

**A self-hosted verification harness runs the cross-compiled host CLI *inside* a
native-arch (aarch64) guest and drives a real, kept-built tart macOS golden's
agent (`:8648`) + VNC through a macOS-host port-forward.** The guest CLI targets
`host-gateway:PORT`; the host forwards (`socat` / `ssh -L`) to the golden.

- **Host-under-test (HUT) VM + provisioning channel — asymmetric by what the
  guest OS offers:**
  - **Linux HUT = stock tart Ubuntu ARM64**, provisioned with only the cross
    binary over **ssh** (reusing the ADR-0007 `russh` layer). `sshd` is
    universal on Linux, so the Linux harness needs no agent and **no dependency
    on the deferred Linux golden** — it is fully self-contained.
  - **Windows HUT = the Windows agent-golden**, provisioned over the **in-VM
    agent's `file upload` + `exec` HTTP surface** — Windows ships no SSH server,
    so the agent is the only in-guest control channel. The Windows harness
    therefore depends on the Windows golden *and* a working Windows agent.

- **Three-band surface split:**
  - **Endpoint-free smoke** (no target): `capabilities`, `schema`,
    `llm-instructions`, `doctor`, `--help`, dry-runs — proves the binary
    *executes* and emits correct envelopes on the target.
  - **Endpoint-driven smoke** (against the forwarded golden): `agent` HTTP
    actions, `input *`, `screen capture`/`size`/`find-text`, `screen
    record`→mp4 — proves the high-risk cross facilities (RFB client, OCR, libav)
    load and run.
  - **Build/compile-only**: `vm start/stop/list/delete` + `vm create-golden`
    (need nested virt / are host-orchestration) — not exercised in-guest.

- **Arch coverage:** aarch64 builds get full in-guest smoke; **x86_64 builds are
  build/link-verified only** (no native x86_64 guest on this Mac) with the
  runtime gap **logged**, never silently treated as covered.

## Considered Options

- **Deterministic stub endpoint on the host** (fake agent HTTP + minimal RFB).
  Reproducible and golden-free — attractive for the no-CI local-release world.
  Rejected as the primary path: it tests a fiction unless kept perfectly
  faithful to the real agent/RFB wire, and the CLI's RFB/agent *client* code is
  already exercised against real endpoints by the macOS `live-vm-gate`. The new
  Tier-2 variable is only "does the cross binary run + speak the wire on this
  OS/arch," which a real endpoint answers honestly. (Retained as a possible
  unit-level fallback.)
- **Guest-loopback** (run an agent + VNC inside the HUT, drive localhost).
  Self-contained, no host networking. Rejected: needs the out-of-scope in-VM
  agent inside the HUT and is artificial.
- **x86_64-only distribution matrix** (matching the `080` spike). Rejected: it
  leaves *nothing* natively runnable in a guest on this Mac, forcing slow QEMU
  TCG emulation or a real x86_64 box for *all* runtime verification — it breaks
  the harness's premise. Promoting aarch64 to first-class restores native
  verifiability.

## Consequences

- The **distribution target matrix is four triples** — `x86_64`/`aarch64` ×
  `linux-gnu`/`windows-{gnu,gnullvm}` — with aarch64 first-class because it is
  the natively verifiable arch. Windows targets use the cross-friendly
  `-gnu`/`-gnullvm` variants (msvc cannot cross from a Mac).
- **Linux verification leads; Windows trails.** The Linux harness is
  self-contained (stock image + ssh); the Windows harness waits on the Windows
  golden + a working Windows agent. Distribution per OS trails that OS's
  host-pass + harness — never ship a binary the harness has not run green.
- The **harness machinery** (host port-forward to a golden, the three-band smoke
  driver, the russh provisioning) is built once for Linux and **reused** for the
  Windows harness with the provisioning channel swapped (ssh → agent).
- **x86_64 runtime stays unverified on this Mac** by construction; the gap is a
  logged, accepted limitation, closable later with a real x86_64 host or
  emulation if it earns its cost.
