---
title: CLI
---

# Component: `cli/` — Host CLI and driver library

Swift package on the macOS host. Produces the `testanyware` executable
and the `TestAnywareDriver` library that embedded Swift callers can
link against.

## Layout

```
cli/
├── Package.swift
├── Sources/
│   ├── testanyware/                      # CLI entry (swift-argument-parser)
│   │   ├── TestAnywareCLI.swift          # Top-level command + ConnectionOptions
│   │   ├── ScreenshotCommand.swift
│   │   ├── ScreenSizeCommand.swift
│   │   ├── InputCommand.swift            # input key/type/click/...
│   │   ├── ExecCommand.swift
│   │   ├── FindTextCommand.swift         # OCR via Server subprocess
│   │   ├── RecordCommand.swift
│   │   ├── AgentCommand.swift            # agent health/snapshot/press/...
│   │   ├── VMCommand.swift               # vm start/stop/list/delete
│   │   └── ServerCommand.swift           # internal _server (hidden)
│   ├── TestAnywareDriver/                # Library (reusable from Swift apps)
│   │   ├── Connection/                   #   ConnectionSpec, Platform parser
│   │   ├── VNC/                          #   VNCCapture, framebuffer converter
│   │   ├── Input/                        #   VNCInput, PlatformKeymap
│   │   ├── Capture/                      #   StreamingCapture (AVAssetWriter)
│   │   ├── Agent/                        #   AgentTCPClient (HTTP to in-VM agent)
│   │   ├── OCR/                          #   Apple Vision + EasyOCR bridge
│   │   ├── Server/                       #   Internal long-running _server + client
│   │   └── VM/                           #   Tart, QEMU+swtpm, lifecycle, paths
│   └── TestAnywareAgentProtocol/         # Wire-format types (host copy)
└── Tests/
    ├── IntegrationTests/                 # Need a live VM; honour TESTANYWARE_SKIP_INTEGRATION
    ├── TestAnywareAgentProtocolTests/    # Wire-shape round-trip tests
    ├── TestAnywareDriverTests/           # Unit tests for the library
    └── Resources/
```

## Key files

| File | Role |
|------|------|
| `cli/Sources/testanyware/TestAnywareCLI.swift` | Top-level `AsyncParsableCommand`. Defines `ConnectionOptions` and the shared resolution chain. |
| `cli/Sources/TestAnywareDriver/Connection/ConnectionSpec.swift` | JSON schema + env-var parsing for connection specs. |
| `cli/Sources/TestAnywareDriver/VNC/VNCCapture.swift` | The single class that wraps the RFB protocol (via vendored RoyalVNCKit). All screenshot, framebuffer, and input code flows through it. |
| `cli/Sources/TestAnywareDriver/VM/VMLifecycle.swift` | `vm start` / `stop` / `list` / `delete` entry points. |
| `cli/Sources/TestAnywareDriver/VM/VMPaths.swift` | XDG path helpers; the authoritative source for on-disk locations. |
| `cli/Sources/TestAnywareDriver/VM/VMSpec.swift` | `<id>.json` writer/reader. Mirrors `ConnectionSpec` + adds `ssh`. |
| `cli/Sources/TestAnywareDriver/Agent/AgentTCPClient.swift` | HTTP/1.1 client for the in-VM agents. |
| `cli/Sources/TestAnywareDriver/Server/TestAnywareServer.swift` | The internal `_server` process that hosts long-running VNC + OCR contexts across multiple CLI invocations. |

## Build / test

```bash
cd cli

# Build
swift build                              # debug
swift build -c release                   # release — binary at .build/release/testanyware

# Unit tests (no VM required)
swift test

# Integration tests (require a running VM — see README integration section)
vmid=$(testanyware vm start)
export TESTANYWARE_VM_ID=$vmid
swift test --filter IntegrationTests
testanyware vm stop "$vmid"

# Skip integration tests explicitly
TESTANYWARE_SKIP_INTEGRATION=1 swift test --filter IntegrationTests
```

## Common pitfalls

- **The `cli/` package is flat.** There is no `cli/macos/` subdir.
  Downstream callers that used to build at `cli/macos/` must update
  to `cli/`.
- **Rust port pending.** Linux host support is blocked on a planned
  Rust port of the driver. Until then, TestAnyware only runs on a
  macOS host. Rationale is captured in
  `LLM_STATE/core/decisions.md`.
- **`TestAnywareAgentProtocol` has two source copies** — one here,
  one under `agents/macos/`. They must stay in sync by hand. A
  round-trip test in `cli/Tests/TestAnywareAgentProtocolTests/`
  catches wire-shape drift.
- **`testanyware _server` is internal**, hidden from help output.
  Never call it directly — the CLI starts it on demand over a UNIX
  socket to reuse the VNC connection across invocations.
- **First-run AppleScript permission:** `testanyware vm start --viewer`
  and `vm stop` need Automation permission on System Events. macOS
  binds the grant to the binary's path — install `testanyware` at a
  stable path (e.g. `/usr/local/bin/testanyware`) if you want to
  rebuild without re-granting.
