# 4. The Rust CLI drops the persistent shared-VNC server

Date: 2026-05-31

## Status

Accepted

## Context

The Swift CLI shipped a hidden `_server` subcommand backed by
`TestAnywareDriver/Server/` (`TestAnywareServer`, `ServerClient`). It was a
**shared-VNC server**: a long-lived host-side process that held **one** VNC
connection open and multiplexed it across many short CLI invocations.

Its shape (`cli/Sources/testanyware/ServerCommand.swift`):

- Started on demand, hidden from `--help` (`shouldDisplay: false`), never
  called directly by users.
- Took a JSON-encoded `ConnectionSpec` and an `--idle-timeout` (default 10s).
- Opened one RFB connection, then listened on a per-spec **unix socket** with
  a **PID file** alongside it (`ServerClient.socketPath(for:)` /
  `.pidPath(for:)`), printed `ready`, and served subsequent `testanyware`
  invocations over that socket until the idle timeout fired and it exited.

The motivation was VNC connection reuse: the RFB handshake has a cost, and a
persistent multiplexer amortized it across a burst of commands. The price was
a stateful daemon with socket/PID lifecycle, idle-timeout teardown, and a
client/server split inside the CLI.

When the surface was ported to Rust, `server` survived only as a clap variant
dispatching to `unimplemented("server")` — a stub shadowing the Swift design,
never built. It is **not** in `surface.rs::CANONICAL_COMMANDS`, so the
`cli-contract.rs` test never asserted it. The decision to drop the persistent
server was made during the port but never recorded as a committed ADR (the
prior record predated the LLM_STATE→grove migration and is not recoverable);
this ADR re-derives it.

## Decision

**The Rust CLI does not have a persistent shared-VNC server, and the `server`
stub is removed.**

Every command opens its own **short-lived RFB connection**: connect →
handshake → act → disconnect. This is already the established norm for the
ported surface — `input *` and `screen *` each open one connection per
invocation (see the header comment in
`cli-rs/crates/testanyware-cli/src/commands/input.rs`: *"Each handler opens
one short-lived RFB connection … then disconnects. This mirrors the Swift
CLI's per-invocation lifecycle."*). There is no multiplexer, no unix socket,
no PID file, no idle-timeout daemon.

What replaces the shared server is therefore **nothing persistent** — just
per-invocation connections. The handshake cost the shared server amortized is
accepted as the cost of a stateless, simpler CLI. The only planned long-lived
RFB consumer is the embedded `egui` viewer (leaf `060-egui-viewer`), and that
is a *viewer* — a single-process display surface — not a shared multiplexer
that other CLI invocations attach to.

Concretely: the `Command::Server` clap variant and its
`Command::Server { .. } => unimplemented("server")` dispatch arm are deleted
from `main.rs`.

## Consequences

- `server` is gone from CLI help, the parser, and dispatch; `cargo build` is
  clean and `cli-contract.rs` passes (it never asserted `server`).
- **This does not retire the OCR daemon / `OcrChildBridge`.** The shared-VNC
  `_server` and the OCR daemon are structurally distinct problems the Swift
  CLI happened to solve with similar helper plumbing: one multiplexes a VNC
  connection, the other keeps a Python EasyOCR process warm because its
  cold-start is multi-second. `OcrChildBridge` (in `testanyware-ocr-client`)
  remains load-bearing scaffold for the Linux/Windows OCR path and the wider
  vision pipeline (ADR-0002). See `CONTEXT.md`, glossary terms *Shared-VNC
  server* and *OCR daemon*. Earlier framing conflated the two; they are not
  the same thing, and only the former is retired here.
- The Swift `_server` and its `Server/` driver code still physically exist in
  `cli/`; the docs describing them (`docs/components/cli.md`,
  `docs/reference/error-codes.md`) remain accurate for the Swift tree and are
  retired wholesale when `cli/` is deleted, not scrubbed piecemeal here.
