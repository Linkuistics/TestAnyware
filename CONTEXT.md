# TestAnyware

TestAnyware is a host-side CLI plus per-platform in-VM agents that drive
isolated guest VMs (Linux / macOS / Windows) for accessibility-API-driven
UI testing. The CLI orchestrates VM lifecycle and acts as a stable,
scriptable surface over the in-VM agents and the VNC framebuffer.

## Language

**Host CLI**:
The user-facing `testanyware` binary that runs on the developer's machine
and orchestrates VMs + agents. Currently in transition: a legacy Swift
implementation in `cli/` is being retired in favour of a Rust port in
`cli-rs/`.
_Avoid_: bare "CLI" — in-VM components also have CLIs.

**Swift CLI**:
The legacy host-CLI implementation under `cli/Sources/testanyware/`. The
retirement target of the `rust-cli-port` grove.
_Avoid_: "the old CLI", "cli/" (use the term, not the path).

**Rust CLI**:
The Rust implementation of the host CLI under `cli-rs/`, structured as a
Cargo workspace (`testanyware-cli` binary + supporting crates). The
in-progress replacement for the Swift CLI.

**Command surface**:
The set of user-visible `testanyware <command>` invocations, plus
hidden/internal subcommands (e.g. `_server`). The canonical list lives in
`cli-rs/crates/testanyware-cli/src/surface.rs`.

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

**Shared-VNC server**:
The Swift `_server` process — a long-lived host-side daemon that holds **one
VNC connection** open on a unix socket (with a PID file and idle timeout) and
multiplexes it across CLI invocations. The Rust CLI **deliberately drops** it:
every command opens its own short-lived RFB connection instead. Do not confuse
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
> **Dev:** Should the Swift CLI behave the same way?
>
> **Maintainer:** Yes — strict parity until `cli/` retires. Whatever
> behavior the contract pins down, both Swift CLI and Rust CLI must
> match.
