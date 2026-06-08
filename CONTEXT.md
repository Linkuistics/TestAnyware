# TestAnyware

TestAnyware is a host-side CLI plus per-platform in-VM agents that drive
isolated guest VMs (Linux / macOS / Windows) for accessibility-API-driven
UI testing. The CLI orchestrates VM lifecycle and acts as a stable,
scriptable surface over the in-VM agents and the VNC framebuffer.

## Language

**Host CLI**:
The user-facing `testanyware` binary that runs on the developer's machine
and orchestrates VMs + agents. Implemented in Rust under `cli-rs/` (the
**Rust CLI**); the legacy Swift implementation was retired 2026-06-03.
_Avoid_: bare "CLI" — in-VM components also have CLIs.

**Swift CLI** (historical):
The original host-CLI implementation, macOS-only, that lived under
`cli/Sources/testanyware/`. **Retired 2026-06-03** when the `rust-cli-port`
grove reached macOS parity and deleted `cli/`; recoverable from git history.
The term survives only because ADRs and docs reference it as the
retirement target.
_Avoid_: present-tense framing — it no longer exists in the tree.

**Rust CLI**:
The Rust implementation of the host CLI under `cli-rs/`, structured as a
Cargo workspace (`testanyware-cli` binary + supporting crates). Since
2026-06-03 it *is* the host CLI — no longer a replacement-in-progress.

**Command surface**:
The set of user-visible `testanyware <command>` invocations, plus any
hidden/internal subcommands. The canonical list lives in
`cli-rs/crates/testanyware-cli/src/surface.rs`. (The Swift CLI's hidden
`_server` subcommand was dropped with the Shared-VNC server — ADR-0004.)

**CLI design contract**:
`docs/architecture/cli-design-contract.md` — the cross-command spec the
Rust CLI is built to satisfy (envelope shape, error/exit codes, dry-run,
help-text template, schema discovery). Retirement of the Swift CLI
requires every command to satisfy this contract.
_Avoid_: bare "the contract" — there are other contracts in adjacent
components.

**In-VM agent**:
A per-platform process running inside a guest VM that exposes
accessibility APIs over HTTP for the Host CLI to drive. Lives under
`agents/{linux,macos,windows}/`. Today three independent implementations
(Linux=Python, macOS=Swift, Windows=C#). **Out of scope** for the
`rust-cli-port` grove — separate workstreams.

**Agent a11y surface**:
The accessibility-tree subset of the *In-VM agent*'s HTTP endpoints, shared
(near-identically) across all three platforms: `windows`, `snapshot`,
`inspect`, `press`, `set-value`, `focus`, `show-menu`,
`window-{focus,resize,move,close,minimize}`, `wait`. Distinct from the agent's
*non-a11y* endpoints (`/exec`, `/upload`, `/download`, `/shutdown`, `/health`),
which are file/process/lifecycle, not accessibility.
_Avoid_: conflating the whole agent with its a11y surface — the agent is the
process + HTTP envelope + both surfaces.

**Golden image**:
A pre-built per-platform VM disk image (`testanyware-golden-<platform>-...`)
that `vm start` clones to spawn a fresh instance.

**Autounattend provisioning** (Windows golden creation):
How the **Windows** Golden image is built — structurally unlike the macOS one.
macOS *clones a pre-built vanilla image* and provisions a running system over
SSH (ADR-0007 `russh`); Windows instead boots a **blank disk from a Microsoft
evaluation ISO** alongside an **autounattend USB** and lets Windows Setup run a
fully **unattended install** (`autounattend.xml`: partitions, bypasses
TPM/SecureBoot/RAM checks, creates `admin`/autologin, installs VirtIO drivers,
registers the *In-VM agent* as a Task Scheduler logon task + Chocolatey).
Post-install provisioning then runs over the **in-VM agent's HTTP surface**
(`/health`, `/exec`) — **Windows ships no sshd**, so the agent is the only
in-guest control channel. The answer file + post-install scripts
(`SetupComplete.cmd`, `desktop-setup.ps1`) are `include_str!`-embedded in the
binary; the agent `.exe` and the VirtIO ARM64 drivers are staged into the
throwaway USB at run time (nothing test-specific is baked into the image). A
**macOS-host** operation: the
FAT32 media is built with `hdiutil`, QEMU+swtpm runs the install. The port lives
in `vm create-golden --platform windows` (grove `220/020`, ADR-0009); it
*documents* this model rather than deciding it (the model predates the port in
`provisioner/helpers/autounattend.xml`).
_Avoid_: assuming the macOS clone-and-SSH model applies to Windows; calling the
autounattend USB a "golden" (it is throwaway install media).

**Shared-VNC server** (historical):
The Swift `_server` process — a long-lived host-side daemon that held **one
VNC connection** open on a unix socket (with a PID file and idle timeout) and
multiplexed it across CLI invocations. The Rust CLI **deliberately dropped** it
(ADR-0004): every command opens its own short-lived RFB connection instead. The
Swift `_server` tree was deleted with `cli/` on 2026-06-03. Do not confuse
with the *OCR daemon* — structurally unrelated (one multiplexes VNC, the other
hosts a Python OCR process); the Swift CLI merely solved both with similar
helper plumbing. The retirement is owned by ADR-0004.
_Avoid_: "the server" (ambiguous — agents and the OCR daemon are also servers),
conflating it with `OcrChildBridge`.

**OCR daemon**:
The long-lived Python child process (`OcrChildBridge` in
`testanyware-ocr-client`) that hosts EasyOCR for the Linux/Windows OCR path,
kept warm because cold-start is multi-second. **Retained** scaffold for the
wider vision pipeline (ADR-0002), distinct from the retired *Shared-VNC
server*.
_Avoid_: calling it "the server".

**Embedded viewer**:
The Rust CLI's `testanyware viewer` command (and the `vm start --viewer`
sugar) — an in-process `eframe`/`wgpu` window that renders a live RFB
`FramebufferUpdate` stream and forwards input to the guest. It was the first
**long-lived RFB consumer** — every other command opens a short-lived
per-invocation connection (ADR-0004) — and the first continuous driver of the
`testanyware-rfb` client. `screen record` is now the **second** long-lived
consumer (ADR-0006): a *bounded, non-interactive* one that samples the stream
into a video encoder for `--duration` seconds, so the viewer is no longer the
*only* one. Architecture in ADR-0005: dedicated RFB thread + isolated runtime,
eframe on the main thread. Contrast the Swift `--viewer`, which launched an
*external* VNC app via AppleScript.
_Avoid_: "the VNC server" (it is a *client*/display surface, not a server);
conflating it with the retired *Shared-VNC server* (a multiplexer other
invocations attached to — the viewer is a single standalone display).

**Self-hosted verification harness**:
The Tier-2 mechanism (ADR-0009) that proves the *cross-compiled* Host CLI
actually runs on Linux/Windows: it runs the binary **inside a native-arch
(aarch64) guest** and has it drive a real tart macOS Golden image's agent +
VNC endpoint through a **macOS-host port-forward** (the guest only ever talks
to the host gateway — the one reliable NAT edge). Verifies **aarch64** builds
natively on the Apple-Silicon Mac; **x86_64** builds are build/link-verified
only (no native x86_64 guest here), with the gap logged. Reuses the ADR-0007
`russh` provisioning on Linux; on Windows, the in-VM agent's `file`/`exec`
surface (no SSH on Windows).
_Avoid_: conflating with the live-VM gate (`tests/live-vm-gate.rs`), which runs
the *macOS* CLI against a macOS golden — the harness is about *non-macOS* host
binaries.

**Host-under-test (HUT) VM**:
The guest VM that *runs* the cross-compiled Host CLI in the self-hosted
verification harness — it is the **host** being tested, not a driven target, so
it needs **no in-VM agent** (the CLI drives a *separate* forwarded endpoint).
Linux HUT = stock tart Ubuntu ARM64 (ssh-provisioned); Windows HUT = the
Windows agent-golden (agent-provisioned, since Windows lacks SSH).
_Avoid_: confusing the HUT (runs the CLI) with the forwarded Golden image
(provides the agent/VNC the CLI drives).

**Host-side framebuffer**:
A VNC/RFB endpoint served by the **hypervisor on the host**, not by software
inside the guest — so it is reachable headless and **before the guest OS
boots** (boot screen, login window, recovery). Both supported backends provide
it: tart via `tart run --vnc-experimental`'s `vnc://` URL, QEMU via a `-vnc`
endpoint backed by its monitor socket. It is the **first gate** any new VM
backend is evaluated against (ADR-0010): the whole `testanyware-rfb` stack, and
golden creation's pre-boot recovery cycle (ADR-0008), depend on it. **Parallels
Desktop was rejected** precisely because it offers no host-side framebuffer —
only a guest-side VNC server, which cannot reach the pre-boot framebuffer.
_Avoid_: conflating with a *guest-side* VNC server (runs inside a booted guest,
can't serve boot/login/recovery) or with the [[Embedded viewer]] (a *client*
that consumes a framebuffer, not a server that provides one).

## Example dialogue

> **Dev:** I broke something — `testanyware vm start` is timing out on
> Windows.
>
> **Maintainer:** Is it the Host CLI hanging, or the in-VM agent not
> responding?
>
> **Dev:** The Host CLI prints the warning about the agent not reaching
> health, then exits 0.
>
> **Maintainer:** Then the in-VM agent is the suspect, not the Host CLI.
> The in-VM agent is out of the rust-cli-port grove's scope — file it
> against `agents/windows/`, not against `cli-rs/`. The Host CLI's job is
> just to clone the Golden image and wait; the CLI design contract says
> it warns and proceeds with `agent: null`.
>
> **Dev:** Where is that behavior pinned down now that the Swift CLI is gone?
>
> **Maintainer:** The CLI design contract is the single source of truth, and
> the Rust CLI — now *the* Host CLI — is built to satisfy it. The
> `cli-contract.rs` integration test is the gate.
