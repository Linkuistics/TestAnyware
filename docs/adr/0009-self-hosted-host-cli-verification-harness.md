# 9. The cross-compiled host CLI is verified by running it inside native-arch guests driving a real golden through a host port-forward

Date: 2026-10-21

## Status

Accepted. First recorded on 2026-06-04. Reworked in grove `gh-issue-8`, leaf
`app-all-guest-traffic-k42`, when both HUTs became goldens that the TestAnyware
app starts. The spec of the app's area is `docs/specs/testanyware-app.md`.

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
guest CLI targets `host-gateway:PORT`. A forward in the harness's own test
process relays each connection to the golden. On a macOS host, the forward opens
a guest stream through the TestAnyware app for each connection (ADR-0004).

- **The host-under-test (HUT) VM is a golden, and the in-VM agent is its
  provisioning channel:**
  - **The Linux HUT is the Linux golden.** The harness starts it with
    `vm start --platform linux`. The clones of the Linux golden run no `sshd`.
    The HUT must be the tart golden. A same-named qcow2 makes the start choose
    QEMU, and the harness then fails and names the qcow2.
  - **The Windows HUT is the Windows golden.** Windows ships no SSH server, so
    the agent is the only control channel in the guest.
  - The harness provisions each HUT through the **`file upload`, `exec` and
    `download` HTTP surface of the in-VM agent**. So each harness depends on its
    golden *and* on a working agent of that golden.
  - On a macOS host, the TestAnyware app starts each HUT and the macOS golden.
    The test process reaches each agent by VM id, as a guest stream, and it
    dials no guest (ADR-0004).

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
- **A stock tart Ubuntu image as the Linux HUT, provisioned over ssh.** This was
  the decision until 2026-10-21. It needed no agent and no Linux golden, which
  was then deferred. Rejected now, because the test process dialled the HUT's
  SSH itself. In a session with no Local Network access, the denial of ADR-0019
  stopped it. To carry that SSH, the app would need a route to port 22 of a VM.
  It would also need to start a VM with no agent. The Linux golden now exists.
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
- **Each harness waits on its golden.** The Linux harness needs the Linux golden
  and the Windows harness needs the Windows golden. Both need a working agent in
  that golden, and a kept-built macOS golden to drive. The distribution of each
  OS trails that OS's host pass and harness. Never ship a binary that the
  harness has not run green.
- The two harnesses have one **harness machinery**. It is the forward to the
  golden, the driver of the three smoke bands, and the agent channel to the HUT.
  Each harness file holds its own copy of it.
- **A check of the libraries takes the place of the stock image.** A stock
  server proved by a run that the binary needs nothing that a server lacks. The
  Linux golden is a desktop, so a run there cannot prove it. Before it stages
  the binary, the Linux harness checks the shared libraries that the binary and
  its bundled libraries need at load time. Each must be a bundled library or a
  part of the C runtime.
- **The same check covers the OCR venv.** The OCR band installs EasyOCR and its
  native wheels in the HUT, as the Linux formula does on a user's host. After
  it provisions the venv, the harness checks each native library in it. A
  library that it needs from outside the venv must be a part of the C or C++
  runtime.
- **The forward binds only the address that the HUT dials.** For a tart HUT it
  accepts only the HUT's own address. For a QEMU HUT it listens on loopback,
  where another local user can reach it during a run. Only the Windows HUT is a
  QEMU HUT. The owner accepted this exposure on 2026-10-21 as a bounded
  exception to the one-user rule, for manual runs of the Windows harness only.
  The spec's section "How the host harnesses reach their guests" states the
  rules and the limits.
- **x86_64 run time stays unverified on this Mac** by construction. The gap is a
  logged and accepted limit. A real x86_64 host or emulation can close it later,
  if it earns its cost.
