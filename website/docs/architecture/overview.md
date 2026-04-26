---
title: Architecture Overview
---

TestAnyware is a two-channel, platform-agnostic driver for VMs. The
host talks to every VM through exactly two wires:

1. **VNC channel** (RFB over TCP) — pixels in, keyboard/mouse out.
2. **Agent channel** (HTTP/1.1 JSON on port 8648) — accessibility tree,
   semantic actions, exec, file transfer, window management.

Everything else in the repo — vision pipeline, provisioner, golden
images, vendored RFB client — exists to feed or consume those two
channels.

## Component map

```
                        HOST (macOS 14+)
   ┌────────────────────────────────────────────────────────────────┐
   │  cli/                                                          │
   │  ┌──────────────┐    ┌────────────────────┐                    │
   │  │ testanyware  │───▶│ TestAnywareDriver  │                    │
   │  │  (CLI bin)   │    │  VNC + Agent +     │                    │
   │  └──────────────┘    │  VM lifecycle      │                    │
   │         │            └──────┬─────┬───────┘                    │
   │         │                   │     │                            │
   │         │         ┌─────────┘     └─────────────┐              │
   │         │         │                             │              │
   │         │   [RFB / port 5900+]       [HTTP / port 8648]        │
   │         │         │                             │              │
   │         ▼         │                             │              │
   │  ┌──────────────┐ │                             │              │
   │  │ provisioner/ │ │                             │              │
   │  │  scripts     │ │   tart / QEMU+swtpm manage VMs below       │
   │  └──────────────┘ │                             │              │
   │                   │                             │              │
   │  vision/          │                             │              │
   │  ┌──────────────┐ │                             │              │
   │  │ stages:      │ │  consumes PNGs captured via VNC            │
   │  │ window/icon/ │ │                             │              │
   │  │ drawing      │ │                             │              │
   │  └──────────────┘ │                             │              │
   └───────────────────┼─────────────────────────────┼──────────────┘
                       │                             │
                       │                             │
           ┌───────────▼───┐          ┌──────────────▼──────────────┐
           │  VM framebuffer│          │  agents/<platform>/         │
           │  (RFB server)  │          │  ┌──────────┐  ┌──────────┐ │
           │                │          │  │  macOS   │  │  linux   │ │
           │  tart: macOS   │          │  │  Swift + │  │  Python+ │ │
           │  tart: Linux   │          │  │  Hbird   │  │  http... │ │
           │  QEMU: Windows │          │  └──────────┘  └──────────┘ │
           └────────────────┘          │        ┌──────────┐         │
                                       │        │  windows │         │
                                       │        │   C#+    │         │
                                       │        │   ASP.   │         │
                                       │        │   NET 9  │         │
                                       │        └──────────┘         │
                                       └─────────────────────────────┘
                                              IN-VM AGENTS
```

## Where each piece lives

| Component | Path | Language | Runs on |
|-----------|------|----------|---------|
| CLI binary | `cli/Sources/testanyware/` | Swift | Host |
| Driver library (VNC + agent client + VM lifecycle) | `cli/Sources/TestAnywareDriver/` | Swift | Host |
| Wire-format types (host copy) | `cli/Sources/TestAnywareAgentProtocol/` | Swift | Host |
| macOS agent | `agents/macos/` | Swift | In-VM |
| Linux agent | `agents/linux/testanyware_agent/` | Python | In-VM |
| Windows agent | `agents/windows/` | C# | In-VM |
| Vision pipeline | `vision/` | Python (uv workspace) | Host |
| Provisioner (VM lifecycle bash wrappers, autounattend XML) | `provisioner/` | Bash + XML | Host |
| Vendored RFB implementation | `vendored/RoyalVNCKit/` | Swift | Host (linked into driver) |

## Isolation notes

- **The macOS agent is self-contained.** It vendors its own copy of the
  `TestAnywareAgentProtocol` source tree rather than path-depending on
  the host CLI package. The agent ships separately as a VM binary;
  keeping its source self-contained means it can be built in any
  working copy without the host CLI present. Both sides must agree on
  the wire shape — see `docs/architecture/agent-protocol.md`.
- **Windows agent is cross-built from macOS.**
  `dotnet build -r win-arm64 --no-self-contained` on the host produces
  the ARM64 Windows binary that ships inside the golden image's
  autounattend payload.
- **No cli/linux or cli/windows.** The CLI package is flat. Linux host
  support is planned via a Rust port — see
  `LLM_STATE/core/decisions.md`.

## What the two channels do and don't do

The VNC channel is the **only** source of pixels and the **only** sink
for raw keyboard/mouse input. It knows nothing about windows, apps, or
accessibility.

The agent channel is the **only** source of semantic structure. It
speaks in windows, roles, and labels; it cannot capture pixels or
synthesize raw input. (It does run `exec` and file transfer, which
conceptually belong to neither channel but were placed with the agent
to keep the VNC channel strictly RFB.)

This split lets the CLI degrade gracefully: without an agent, you still
have screenshots, video, OCR (via `find-text`), and every keyboard/mouse
primitive. Adding the agent is purely additive.
