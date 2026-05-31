# 060-egui-viewer

**Kind:** planning

## Goal

Plan the embedded **`egui` VNC viewer** that replaces the Swift CLI's external
AppleScript launcher behind `vm ... --viewer`. This is a planning leaf: the
viewer is a fresh subsystem (window/event loop, framebuffer rendering, input
forwarding, connection lifecycle) too large to scope blind. **Grill, then grow
the tree** (decompose into work leaves); do not start implementing here.

## Context

- Today's `--viewer` (Swift) just launches an *external* VNC app via AppleScript
  (`cli/Sources/testanyware/VMCommand.swift`, `var viewer: Bool`). The Rust
  target is an *embedded* viewer — a real RFB client rendering into an `egui`
  window — so this is **beyond Swift parity**, an enhancement, not a port.
- It is the **only planned long-lived RFB consumer** (contrast the retired
  shared-VNC `_server`, leaf 020, and the short-lived per-invocation connections
  used by `input`/`screen`). The viewer holds one connection open and renders a
  continuous `FramebufferUpdate` stream — the RFB client's first long-lived use.
- Builds directly on the RFB client crate: framebuffer decode (Raw/CopyRect +
  ZRLE/Tight from leaf 040) for rendering, and `testanyware-rfb::input` for
  forwarding mouse/keyboard back to the guest.

## Grilling questions to open with

- **Scope**: read-only viewer first, or interactive (input-forwarding) from the
  start? Interactive reuses the input layer but adds focus/coordinate mapping.
- **egui integration**: `eframe`/winit standalone window? Threading model
  between the blocking RFB read loop and the egui paint loop (channel? async
  task + `egui::Context::request_repaint`)?
- **Connection lifecycle**: launched inline by `vm start --viewer`, or a
  separate `testanyware ... viewer` subcommand? How does it resolve the VNC
  endpoint (reuse `resolve_vnc`)?
- **Platform reach**: macOS-only first (matches current primary host), or
  cross-platform from the outset (winit is portable; ties into the later
  Windows-host work)?
- **Encoding dependency**: does the viewer require ZRLE/Tight (040) for usable
  performance, making 040 a hard prerequisite, or is Raw/CopyRect acceptable
  for a first cut?

## Done when

- The viewer's scope and architecture are agreed via grilling; cross-cutting
  decisions captured (CONTEXT.md inline; an ADR only if hard-to-reverse — e.g.
  the egui/eframe + threading-model choice likely qualifies).
- This leaf is decomposed into ordered work leaves (or a node) sized for one
  session each.
- This planning leaf is committed and retired.

## Notes

`egui`/`eframe` is a new top-level dependency — weigh it against the
local-release-from-`scripts/` model (no CI; binary size; cross-compile matrix in
`Cargo.toml`). Relates to backlog "task 8" (viewer) in old code comments —
descriptions only, not status.
