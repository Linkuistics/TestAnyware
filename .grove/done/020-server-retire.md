# 020-server-retire

**Kind:** work

## Goal

Retire the `server` command from the Rust CLI. It is the Rust shadow of the
Swift `_server` — a **shared-VNC persistent server** that holds one long-lived
VNC connection on a unix socket (with PID file + idle timeout) to multiplex it
across CLI invocations. The Rust architecture deliberately dropped that design;
the stub should go, not be ported. Re-record the (currently undocumented)
"Rust CLI drops the persistent server" decision as a fresh ADR.

## Context

- Stub to remove: `cli-rs/crates/testanyware-cli/src/main.rs` — the
  `Command::Server { port }` clap variant (~line 810) and its dispatch arm
  `Command::Server { .. } => unimplemented("server")` (~line 1681). Delete the
  variant and the arm; do not leave a dangling `unimplemented`.
- `server` is **not** in `surface.rs::CANONICAL_COMMANDS` (verified), so the
  `cli-contract.rs` test does not assert it — removal should not break the
  contract test, but re-run it to confirm.
- Swift reference (the thing being retired, for the ADR's "what it was"):
  `cli/Sources/testanyware/ServerCommand.swift` (the `_server` subcommand) and
  `cli/Sources/TestAnywareDriver/Server/` (`TestAnywareServer`, `ServerClient` —
  socket path / pid path / idle-timeout multiplexer).
- **Key distinction the ADR must preserve** (this is the whole reason for the
  leaf): the shared-VNC `_server` is retired; the **OCR daemon pattern**
  (`OcrChildBridge` in `testanyware-ocr-client`) is *not* — they are
  structurally different problems the Swift CLI happened to solve with similar
  helpers. See `CONTEXT.md` (glossary terms *Shared-VNC server* and *OCR
  daemon*) and ADR-0002.
- The prior ADR that recorded this (`0001-rust-cli-drops-persistent-server.md`
  per the `ocr-bridge-is-scaffold-not-residue` memory) is **not recoverable** as
  a file in this repo's history — it predates the LLM_STATE→grove migration and
  was never committed under that path. Re-derive the decision; don't hunt for it.
- `cli-rs/crates/testanyware-ocr-client/src/engine.rs` references the
  distinction in a comment — worth reading for the in-tree framing.

## Done when

- `Command::Server` and its dispatch arm are gone from `main.rs`; `cargo build`
  is clean and `cli-contract.rs` passes.
- No remaining references to a `server`/`_server` host subcommand in CLI help,
  `docs/reference/`, or `docs/components/cli.md` (grep and fix any).
- A new ADR (`docs/adr/0004-rust-cli-drops-persistent-vnc-server.md`) records:
  what the shared-VNC `_server` was, why the Rust CLI drops it (short-lived
  per-invocation RFB connections replace it — see the `input` command lifecycle
  comment in `commands/input.rs`), and an explicit **"does not retire the OCR
  daemon / `OcrChildBridge`"** carve-out.
- Committed as one focused commit.

## Notes

Short-lived RFB connections are already the norm for `input *` and `screen *`
(each opens, acts, disconnects — see `commands/input.rs` header comment). The
ADR's "what replaces it" answer is: nothing persistent; per-invocation
connections. The embedded `egui` viewer (leaf 060) is the *only* planned
long-lived RFB consumer, and it is a viewer, not a shared multiplexer.
