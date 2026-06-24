# 14. macOS guest display resolution via a runtime, agent-mediated mode switch

Date: 2026-06-24

## Status

Accepted — decision made; **implementation deferred to a dedicated grove**
(suggested name `macos-guest-resolution`).

## Context

ADR-0013 introduced a TestAnyware-owned default guest display resolution of
**1920×1080 px**, applied per-backend at `vm start`. Its empirical verification
(2026-06-24) found that the host-side mechanism is **sufficient for QEMU and for
Linux-on-tart but insufficient for macOS-on-tart**: a macOS Virtualization.framework
guest's framebuffer is **guest-controlled**. WindowServer restores the guest's
*own saved display mode* on login (1024×768, baked into the golden) and VF sizes
the framebuffer to it; the host-side `tart set --display 1920x1080px` is accepted
and stored as the VM's display *configuration* (a ceiling / available-mode hint)
but does **not** force the guest to switch modes. So macOS renders 1024×768 —
**off** the vision training distribution (`_SCREEN_W=1920, _SCREEN_H=1080`).

Reaching 1920×1080 px on macOS therefore requires changing the **guest's** chosen
mode, which ADR-0013 scoped out. This ADR records the mechanism for doing so.

**A premise correction.** ADR-0013's verification note and the planning leaf
`macos-guest-resolution-k3` both asserted that a guest-side approach was blocked
because "the agent is UI/accessibility-only (no generic exec)." **This is false.**
The in-guest agent exposes a generic `POST /exec` endpoint that runs `/bin/bash -c`
with stdout/stderr capture (`agents/macos/Sources/testanyware-agent/AgentServer.swift`,
`…/TestAnywareAgent/ProcessRunner.swift`); it is host-reachable
(`cli-rs/crates/testanyware-cli/src/commands/file.rs` calls `client.exec(...)`),
runs as a LaunchAgent on every booted golden, and `CONTEXT.md` already documents
`/exec` as part of the agent's HTTP surface — explicitly warning against
"conflating the whole agent with its a11y surface." sshd (Remote Login) is off in
the running golden, but the agent's HTTP channel (port 8648) is up. A guest-side
exec channel therefore **already exists**; the only missing piece is a
display-mode-switch primitive (the repo has no `displayplacer` and no CoreGraphics
display-mode code — only `set-wallpaper.swift`, which sets wallpaper, not mode).

## Decision

**Set the macOS guest's resolution at runtime, host-orchestrated through the
existing agent channel — not by baking it into the golden image.**

At `vm start` on macOS, after the agent is ready, the host:

1. `/upload`s a small **host-compiled CoreGraphics helper** (`CGDisplayCopyAllDisplayModes`
   → match the target px size → `CGDisplaySetDisplayMode`), mirroring the existing
   host-compiled `set-wallpaper.swift` helper pattern;
2. `/exec`s it with the resolved resolution — the **1920×1080 px** default, or the
   user's `--display` value;

forcing the framebuffer to the target size. **No golden-image change is required**
— the mechanism reuses the agent's existing `/upload` + `/exec` surface. Resolution
stays a **per-instance, runtime** concern, consistent with ADR-0013's scope
principle ("the running test instance is where resolution matters").

This is a **design decision only**; implementation — including the empirical
de-risking below — is deferred to a dedicated grove.

## Considered options

- **Golden-bake the saved WindowServer mode** (set the guest's
  `com.apple.windowserver.displays.plist` saved mode to 1920×1080 during golden
  creation, restored on login). *Rejected:* its correctness depends on an
  **unverified, fragile** assumption — that the saved mode (keyed by display
  identity) survives clone + `tart set --display`; if VF regenerates the virtual
  display's identity per clone the baked plist silently fails. It also contradicts
  ADR-0013's runtime-concern principle and requires **regenerating the kept-built
  golden**. Its only advantages (zero runtime cost, no readiness race) do not
  outweigh the correctness risk.
- **A native `/set-display` agent endpoint** (the agent links CoreGraphics already).
  *Rejected for now:* it requires **regenerating the golden** (the agent binary is
  baked in) and couples the agent to resolution policy. The upload+exec helper
  reaches the same CoreGraphics calls with no golden change. (A native endpoint may
  be reconsidered by the new grove if the per-start upload proves to be a problem.)

## Consequences

- macOS guests will reach a uniform 1920×1080-px framebuffer like the other
  backends, **without** regenerating the macOS golden.
- `vm start` gains a macOS-specific post-boot step: it must **wait for agent
  readiness** (today the agent endpoint is treated as optionally-degraded —
  `tart.rs:315`) and then upload+exec the helper. There is a brief transient where
  the framebuffer is still 1024×768 before the switch; **vision / `screen`
  consumers must gate on the switch completing**, the same way they gate on
  readiness.
- **Empirical questions the new grove must resolve first** (the runtime approach
  fails if #1 is "no"):
  1. Does VF advertise a **selectable** 1920×1080 CoreGraphics display mode for a
     headless guest after `tart set --display 1920x1080px`? If not,
     `CGDisplaySetDisplayMode` has no mode to pick — the new grove would need a
     different primitive (a private CGS mode-creation API) or would fall back to
     golden-bake. This is the load-bearing unknown; de-risk it with a spike before
     building.
  2. The exact CG call sequence and how it behaves headless (no attached display
     session).
  3. Whether the helper should be uploaded per-start or cached in the guest, and
     how `vm start` sequences the agent-readiness wait against the resolution
     switch.
- Relationship to ADR-0013: this ADR *extends* ADR-0013 for the macOS backend; it
  does not supersede it. The per-backend `vm start` default and the tart `px`
  encoding stand. Linux + Windows are fully covered by ADR-0013; macOS is covered
  by ADR-0013 (the host-side `tart set`) **plus** this ADR (the guest-side switch).
