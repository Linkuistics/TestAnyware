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
`agents/{linux,macos,windows}/`. **Out of scope** for the `rust-cli-port`
grove — separate workstreams.

**Golden image**:
A pre-built per-platform VM disk image (`testanyware-golden-<platform>-...`)
that `vm start` clones to spawn a fresh instance.

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
