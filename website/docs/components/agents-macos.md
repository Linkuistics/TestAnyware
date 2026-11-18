---
title: Agents macOS
---

# Component: `agents/macos/` — macOS in-VM agent

This Swift package runs **inside** a macOS VM. It serves the TestAnyware
agent HTTP surface on port 8648 through Apple's native accessibility APIs
(`ApplicationServices` / `AXUIElement`). The transport is
[Hummingbird](https://hummingbird.codes).

## Layout

```
agents/macos/
├── Package.swift
├── Sources/
│   ├── testanyware-agent/                # Entry point (executable)
│   │   └── AgentServer.swift             # Router + endpoint registration
│   ├── TestAnywareAgent/                 # Library (endpoint handlers, AX walkers)
│   └── TestAnywareAgentProtocol/         # Self-contained copy of the wire types
└── Tests/
```

## Key design notes

- **Self-contained.** The package does not path-depend on the host CLI's
  copy of `TestAnywareAgentProtocol`. It vendors its own copy of the Swift
  sources, so the agent builds in any working copy without the `cli/`
  package.
- **Single-process.** One Hummingbird `Application` binds `0.0.0.0:8648`
  and dispatches to the handler closures in `AgentServer.swift`. A second
  argument names another listen address. The host tests pass `127.0.0.1`.
- **No accessibility state between requests.** Each accessibility handler
  walks the AX tree again. When AX is not ready, the client must retry.
- **Exec registry.** The agent registers each exec `id`. It retains each
  finished record and each tombstone for 300 s, 64 at most.
  `/exec/cancel` ends a command through its lifecycle owner. A disconnect
  is not a cancellation (ADR-0025). A waiting cancellation returns the record
  of its own command, also after eviction and reuse of the id. When the
  cancellation pipe or its read source fails, the owner polls for the
  request every 10 ms.
- **One file version for each download.** A download opens the file once.
  It sends exactly the length that `fstat` gave for that handle
  (ADR-0026). The open does not block. A path that is not a regular file,
  such as a FIFO without a writer, gets 404 at once.
- **Bounded shutdown.** At SIGTERM the agent stops admission and cancels
  each running guest command. It stops accepting connections, waits up to
  4 s for the records and then exits with status 0. A second SIGTERM ends
  it at once. The spec is `docs/specs/guest-command-lifecycle.md`.

## Endpoint wiring

All routes are in
`agents/macos/Sources/testanyware-agent/AgentServer.swift`:

```
GET  /health
POST /windows, /snapshot, /inspect
POST /press, /set-value, /focus, /show-menu
POST /window-focus, /window-resize, /window-move, /window-close, /window-minimize
POST /wait
POST /exec, /exec/cancel, /upload, /download, /shutdown
POST /debug/ax    (macOS-only diagnostic)
```

`docs/architecture/agent-protocol.md` documents the wire shapes.

## Build / test

Build **inside the VM**, or build on the host for installation into the
golden image:

```bash
cd agents/macos
swift build -c release
# Binary ends up at .build/release/testanyware-agent
swift test
```

The macOS golden holds the binary at `/usr/local/bin/testanyware-agent`.
A LaunchAgent with the label `com.linkuistics.testanyware-agent` starts it.

## Common pitfalls

- **The TCC grant requires a SIP cycle.** The macOS golden-image creation
  script boots into Recovery and disables SIP. It writes a direct TCC
  database row that grants AX to the binary with its code-signing
  requirement. Then it enables SIP again. A change to the binary after
  installation makes the grant invalid, so the installed path and the
  signature are load-bearing.
- **The AX tree is not ready at login.** Directly after desktop login, AX
  calls can return empty trees for several seconds. Call `POST /wait` first
  in scripts that run directly after boot.
- **Window origin and the CLI inset.** macOS 27 reports the visible top of
  a window as its AX origin. By default, the CLI subtracts a 40 px top
  inset from it. See
  [`docs/user/troubleshooting.md`](../user/troubleshooting.md).
- **Keep the protocol copy in sync.** When you change a wire shape, make
  the same change in `cli-rs/crates/testanyware-protocol/`. The fixtures
  contract test in `cli-rs/crates/testanyware-protocol/tests/fixtures.rs`
  and its agent-side counterpart catch a drift.
