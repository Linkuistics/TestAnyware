# 060-swift-cli-streaming

**Kind:** work

## Goal

Make the Swift CLI (`cli`) upload/download stream per ADR-0001, keeping strict
wire parity with the Rust client (CONTEXT.md: "strict parity until cli/
retires"). Even though the Swift CLI is being retired, it must match the new
contract or it breaks.

## Context

- `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift` — `upload(path:
  content:)` (~180) posts JSON via `postJSON("/upload", ...)`. Replace with a
  streaming POST to `/upload?path=<encoded>` carrying raw bytes; add/adjust the
  download method to consume an octet-stream response.
- `cli/Sources/testanyware/ExecCommand.swift` — `UploadCommand` (~27) reads the
  local file into `Data` (~41) and calls `agent.upload`. Stream from disk
  instead of loading whole `Data` where the HTTP client allows.
- `cli/Sources/TestAnywareAgentProtocol/` — remove/repurpose `UploadRequest`/
  `DownloadRequest`; keep in lockstep with the macOS vendored copy (leaf 020)
  and the protocol-drift test (`cli/Tests/TestAnywareAgentProtocolTests/`).
- ADR-0001 + rewritten agent-protocol.md (leaf 010) + the Rust client (leaf 050)
  as the parity reference.

## Done when

- Swift `upload`/`download` speak the identical octet-stream + `?path=` wire
  form as the Rust client.
- `UploadRequest`/`DownloadRequest` JSON types removed; the protocol-drift test
  passes against the updated contract.
- `cli` builds; `UploadCommand`/download command behavior preserved
  (same printed output, same errors).

## Notes

Swift's URLSession supports streaming upload (`uploadTask(with:fromFile:)`) and
streamed download to a file — prefer those over loading `Data`. If the existing
`AgentTCPClient` is a hand-rolled socket client, match its style; the goal is
bounded memory, not necessarily URLSession.
