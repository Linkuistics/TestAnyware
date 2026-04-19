# Component: `agents/macos/` — macOS in-VM agent

Swift package that runs **inside** a macOS VM. Exposes the TestAnyware
agent HTTP surface on port 8648 using Apple's native accessibility APIs
(`ApplicationServices` / `AXUIElement`). Transport is
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

- **Self-contained** — deliberately does not path-depend on the host
  CLI's copy of `TestAnywareAgentProtocol`. Vendors its own copy of
  the Swift sources so the agent builds in any working copy without
  requiring the `cli/` package.
- **Single-process** — one `AsyncHTTPServer` binds `0.0.0.0:8648` and
  dispatches to handler closures in `AgentServer.swift`.
- **No state between requests** — each handler walks the AX tree
  fresh. Clients responsible for retries when AX isn't ready.

## Endpoint wiring

All routes live in
`agents/macos/Sources/testanyware-agent/AgentServer.swift`:

```
GET  /health
POST /windows, /snapshot, /inspect
POST /press, /set-value, /focus, /show-menu
POST /window-focus, /window-resize, /window-move, /window-close, /window-minimize
POST /wait
POST /exec, /upload, /download, /shutdown
POST /debug/ax    (macOS-only diagnostic)
```

Wire shapes are documented in `docs/architecture/agent-protocol.md`.

## Build / test

**Inside the VM** (or cross-built on the host for installation into the
golden image):

```bash
cd agents/macos
swift build -c release
# Binary ends up at .build/release/testanyware-agent
swift test
```

The binary is installed at `/usr/local/bin/testanyware-agent` inside
the macOS golden and launched by a LaunchAgent with label
`com.linkuistics.testanyware-agent`.

## Common pitfalls

- **TCC grant requires SIP cycle.** The macOS golden-image creation
  script boots into Recovery, disables SIP, writes a direct TCC
  database row granting AX to the binary with its code-signing
  requirement, then re-enables SIP. Modifying the binary after
  installation invalidates the grant — the installed path and
  signature are load-bearing.
- **AX tree not ready at login.** Immediately after desktop login,
  AX calls can return empty trees for up to several seconds. Call
  `POST /wait` first in scripts that run right after boot.
- **Tahoe drop-shadow inset.** AX-reported window origin includes the
  drop-shadow inset (~40 px). See
  [`docs/user/troubleshooting.md`](../user/troubleshooting.md).
- **`NSStackView` element resolution.** Text fields hosted inside
  `NSStackView` containers on Tahoe may not appear via
  `kAXChildrenAttribute`. Workaround in troubleshooting.md.
- **Keep the protocol copy in sync.** If you change wire shapes, apply
  the same change in `cli/Sources/TestAnywareAgentProtocol/`. The
  round-trip test in
  `cli/Tests/TestAnywareAgentProtocolTests/` is the safety net.
