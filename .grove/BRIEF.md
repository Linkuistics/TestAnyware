# fix-agent-upload-is-capped-at-8MB — root brief

## Problem

`testanyware file upload` fails for files larger than ~8 MB. The reported
"8 MB cap" is the user-visible symptom of a macOS-specific request-body
limit, compounded by base64 inflation.

## Diagnosis (from codebase, 2026-05-29)

The `/upload` endpoint (see `docs/architecture/agent-protocol.md`) sends the
whole file base64-encoded inside a JSON body:
`{ "path": "...", "content": "<base64>" }`. Base64 inflates payload size by
4/3, so the effective *file* cap is ¾ of whatever the *request-body* cap is.

Effective caps differ per agent because each uses a different HTTP stack:

| Agent   | Stack                     | Request-body cap        | Effective file cap | Source |
|---------|---------------------------|-------------------------|--------------------|--------|
| macOS   | Hummingbird               | 10 MiB (`collect(upTo: 10_485_760)`) | **~7.5 MiB** ← the reported cap | `agents/macos/Sources/testanyware-agent/AgentServer.swift:106` |
| Windows | ASP.NET / Kestrel         | ~28.6 MiB (Kestrel default, not overridden) | ~21 MiB | `agents/windows/Program.cs` (no `Limits` set) |
| Linux   | stdlib `http.server`      | none (reads full `Content-Length`) | memory-bound only | `agents/linux/testanyware_agent/server.py:62` |

Both CLI clients read the entire file into memory before encoding:
- Rust: `std::fs::read(local_path)` — `cli-rs/.../commands/file.rs:105`
- Swift: `Data(contentsOf:)` then `agent.upload` — `cli/Sources/testanyware/ExecCommand.swift`

So three problems are tangled together:
1. **The binding bug:** macOS caps files at ~7.5 MiB.
2. **Inconsistency:** three different, undocumented effective caps.
3. **Memory ceiling:** whole-file base64-in-JSON on both ends does not scale to
   large files regardless of where the cap is set.

## Scope

To be settled by grilling (see `010-*`). The decision among "raise the macOS
cap", "unify a documented cap across all agents", and "redesign upload to
stream/chunk" determines the shape of this grove's tree.

## Decisions (running log)

**Q1 — Scope (settled 2026-05-29): Redesign upload to stream/chunk.**
Not a cap bump and not merely unification — the goal is to remove the
whole-file-in-memory ceiling entirely so uploads scale to arbitrarily large
files. This is a coordinated protocol change spanning the agent protocol doc +
all three agents (Hummingbird / Kestrel / `http.server`) + both CLI clients
(Rust `cli-rs`, Swift `cli`) + the macOS agent's vendored protocol copy. Implies
the grove root decomposes into a node. Open sub-questions: wire model
(raw-binary stream vs. chunked app-protocol vs. base64-chunked), endpoint shape
& Swift-parity, download symmetry, write atomicity.

**Q2 — Wire model (settled 2026-05-29): Raw binary streaming body.**
`/upload` takes `Content-Type: application/octet-stream`; the agent streams the
request body straight to a temp file and atomically renames into place. No
base64 (drops the 4/3 inflation and encode/decode CPU), no full-file buffering
on either end — bounded memory regardless of file size, in a single request.
**Resumability is explicitly NOT a requirement**, so the chunked begin/chunk/
commit protocol is rejected as unnecessary complexity. The destination path can
no longer ride in the JSON body; where it rides (header vs. query) is the next
question.

**Q3 — Path transport (settled 2026-05-29): percent-encoded query parameter.**
`POST /upload?path=<percent-encoded>` and `POST /download?path=<percent-encoded>`.
Query-param percent-encoding has unambiguous, well-supported rules across all
three stacks and handles Unicode / special characters in guest paths; HTTP
header values (ASCII/Latin-1 by spec) would need a bespoke encoding convention.
Endpoint *names* (`/upload`, `/download`) stay, so the protocol doc's endpoint
table is stable.

**Q4 — Download symmetry (settled 2026-05-29): stream download too.**
`/download` stops returning base64-in-JSON and instead streams the file as an
`application/octet-stream` response body; failures return a JSON `ErrorResponse`
distinguished by HTTP status (reusing the existing `download_failed` key). Both
halves of file transfer scale; no half-fixed memory ceiling left behind.

**Q5 — Write safety (settled 2026-05-29): temp file + atomic rename.**
Each agent streams into a temp file in the *same directory* as the destination,
then renames into place only on a complete, successful transfer; any error
unlinks the temp file. The destination path therefore never holds a truncated
file. Cost: transient ~2x disk in the target directory; rename must stay on one
filesystem (sibling temp guarantees that).

**Q6 — Cutover (settled 2026-05-29): hard coordinated cutover, no fallback.**
All three agents + both CLI clients + the protocol doc + the macOS vendored
protocol copy change together; golden images are rebuilt. No capability
negotiation and no base64 fallback path — keeping one would defeat the
ceiling-removal goal and leave dead code in a soon-to-be-retired Swift CLI.
Mismatched CLI/agent versions fail with a clear error. Accepted because this is
a single monorepo with no external protocol consumers.

## Decision record

Captured durably in **ADR-0001** (`docs/adr/0001-streaming-file-transfer.md`) —
the normative contract sketch that the protocol-doc and implementation leaves
transcribe.

## Tree

Root decomposes into ordered work leaves (see the leaves alongside this brief).
