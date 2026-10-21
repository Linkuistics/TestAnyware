# 9. The cross-compiled host CLI is verified by running it inside native-arch guests driving a real golden through a host port-forward

Date: 2026-06-04

## Status

Accepted

## Context

Tier 2 of the `rust-cli-port` grove ships the `testanyware` host CLI for Linux
and Windows. The Swift CLI was macOS-only. An arm64 Mac cross-compiles the CLI
locally with `cargo-zigbuild`, and the `080` spike proved that toolchain. A green
cross-*build* does not prove that the binary *runs* on the target. These parts
can fail only at run time, on the target OS and arch:

- the dynamic loader and the glibc floor
- the per-platform OCR engine (EasyOCR through the OCR daemon)
- the `ffmpeg-next` libav link
- the Vulkan and wgpu stack, which the binary loads with `dlopen`.

TestAnyware's own VM machinery is the natural way to get a real target host for
the binary: "test the host CLI with the product."

Two facts shape the design:

1. **Architecture mismatch.** Every guest that this Apple-Silicon Mac boots
   *natively* is **ARM64**: tart Ubuntu ARM64, and Windows 11 ARM64 under
   `qemu-system-aarch64` and swtpm. The `080` spike built **x86_64** binaries.
   An x86_64 ELF or PE cannot run on an ARM64 guest. So only **aarch64** builds
   can run in a guest here. This is why the distribution matrix makes
   `aarch64-pc-windows-*` first-class: it is the *only* Windows build that the
   harness can run. x86_64 Windows runs only on real x86_64 Windows.

2. **No reliable guest-to-guest network.** Under NAT virtualisation, a guest's
   default gateway *is* the host. So the guest can reach anything that is bound
   on the host. Guest-to-*other-guest* routing is not guaranteed without bridged
   networking, for example from a QEMU Linux guest to a tart macOS VM.

The CLI surface that needs an endpoint (agent HTTP, input and screen over RFB,
and OCR) needs a live agent and VNC endpoint to drive. The in-VM agent and the
goldens are a *separate* workstream, outside this scope, and Tier 2 does not
change them. So the variable under test is the **cross-compiled client binary**,
not the endpoint.

## Decision

**A self-hosted verification harness runs the cross-compiled host CLI *inside* a
native-arch (aarch64) guest.** The CLI drives the agent (`:8648`) and VNC of a
real, kept-built tart macOS golden through a port-forward on the macOS host. The
guest CLI targets `host-gateway:PORT`, and the host forwards (`socat` or
`ssh -L`) to the golden.

- **The host-under-test (HUT) VM and its provisioning channel differ by what the
  guest OS offers:**
  - **The Linux HUT is stock tart Ubuntu ARM64.** The harness provisions only
    the cross binary into it, over **ssh**, and reuses the ADR-0007 `russh`
    layer. Every Linux has `sshd`. So the Linux harness needs no agent and **no
    Linux golden**, which is deferred. It is fully self-contained.
  - **The Windows HUT is the Windows agent-golden.** The harness provisions it
    through the **`file upload` and `exec` HTTP surface of the in-VM agent**.
    Windows ships no SSH server, so the agent is the only control channel in the
    guest. So the Windows harness depends on the Windows golden *and* on a
    working Windows agent.

- **The surface splits into three bands:**
  - **Endpoint-free smoke** (no target): `capabilities`, `schema`,
    `llm-instructions`, `doctor`, `--help` and dry-runs. It proves that the
    binary *executes* on the target and emits correct envelopes.
  - **Endpoint-driven smoke** (against the forwarded golden): `agent` HTTP
    actions, `input *`, `screen capture`, `size` and `find-text`, and `screen
    record` to mp4. It proves that the cross facilities of high risk (the RFB
    client, OCR and libav) load and run.
  - **Build and compile only**: `vm start`, `stop`, `list` and `delete`, and
    `vm create-golden`. They need nested virtualisation or they orchestrate the
    host, so no guest runs them.

- **Arch coverage:** aarch64 builds get the full smoke in a guest. **x86_64
  builds are build-verified and link-verified only**, because this Mac has no
  native x86_64 guest. The harness **logs** the gap in run-time coverage, and
  never treats it as covered.

## Considered Options

- **A deterministic stub endpoint on the host** (a fake agent HTTP server and a
  minimal RFB server). It is reproducible and needs no golden, which suits a
  local release with no CI. Rejected as the primary path. It tests a fiction
  unless it stays faithful to the real agent and RFB wire. The macOS
  `live-vm-gate` already runs the CLI's RFB and agent *client* code against real
  endpoints. The new Tier-2 variable is only whether the cross binary runs and
  speaks the wire on this OS and arch. A real endpoint answers that honestly.
  The stub stays a possible fallback at unit level.
- **Guest loopback** (an agent and VNC run inside the HUT, and the CLI drives
  localhost). It is self-contained and needs no host networking. Rejected: it
  needs the in-VM agent inside the HUT, which is outside this scope, and it is
  artificial.
- **A distribution matrix of x86_64 only**, as the `080` spike built. Rejected.
  It leaves *nothing* that runs natively in a guest on this Mac. All run-time
  verification would then need slow QEMU TCG emulation or a real x86_64 box, so
  it breaks the premise of the harness. A first-class aarch64 restores native
  verification.

## Consequences

- The **distribution target matrix has four triples**: `x86_64` and `aarch64`,
  each with `linux-gnu` and `windows-{gnu,gnullvm}`. aarch64 is first-class,
  because it is the arch that the harness can run natively. The Windows targets
  use the `-gnu` and `-gnullvm` variants, which cross-compile easily. msvc cannot
  cross-compile from a Mac.
- **Linux verification leads, and Windows trails.** The Linux harness is
  self-contained, with a stock image and ssh. The Windows harness waits on the
  Windows golden and on a working Windows agent. The distribution of each OS
  trails that OS's host pass and harness. Never ship a binary that the harness
  has not run green.
- The Linux harness builds the **harness machinery** once: the host port-forward
  to a golden, the driver of the three smoke bands, and the russh provisioning.
  The Windows harness **reuses** it, with ssh swapped for the agent as the
  provisioning channel.
- **x86_64 run time stays unverified on this Mac** by construction. The gap is a
  logged and accepted limit. A real x86_64 host or emulation can close it later,
  if it earns its cost.
