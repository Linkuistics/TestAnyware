# 14. macOS guest display resolution via a runtime, agent-mediated mode switch

Date: 2026-06-24

## Status

Accepted and **implemented** (2026-06-24, grove `macos-guest-resolution`,
build leaf `build-resolution-switch-k3`). The spike `spike-display-modes-k2`
CONFIRMED the mechanism (see Verification), and the runtime switch now ships in
`vm start` — see Implementation.

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

## Verification (2026-06-24, `spike-display-modes-k2`)

**Verdict: CONFIRMED** — a selectable 1× 1920×1080 CoreGraphics mode exists, and a
persistent display-configuration transaction switches the host-side framebuffer to
it. The runtime, agent-mediated mechanism is viable; the build leaf
(`build-resolution-switch-k3`) is unblocked. **One material correction** to the
decision above: the bare `CGDisplaySetDisplayMode(_, _, nil)` named in the
Decision is **insufficient on its own** (see finding 3) — the helper must use a
configuration *transaction*.

Method: a host-compiled CoreGraphics probe
(`provisioner/helpers/probe-display-modes.swift`) uploaded through the agent's
`/upload`+`/exec` surface and run inside a **fresh clone** of the live
`testanyware-golden-macos-tahoe`, started with `tart set --display 1920x1080px`
(confirmed by `tart get`: `Display 1920x1080px`). The probe enumerates
`CGDisplayCopyAllDisplayModes` (both with and without
`kCGDisplayShowDuplicateLowResolutionModes`), switches to the 1× target, and
re-reads the active mode; framebuffer reads are `testanyware screen size` (the
negotiated RFB ServerInit — the contract).

**Findings:**

1. **VF advertises 12 modes for the main display, every one at backing scale 1.0
   (LoDPI). There are no Retina/2× modes at all** — the default list and the
   `kCGDisplayShowDuplicateLowResolutionModes` list are identical (12 = 12). All
   modes report `pixelWidth == width` (px == pt):

   | modeID | px (= pt) | flags | usableForDesktopGUI |
   |---|---|---|---|
   | 0 | 800×600 | valid,safe | yes |
   | 1 | 960×540 | valid,safe | yes |
   | 2 | 1024×576 | valid,safe | yes |
   | 3 | **1024×768** | valid,safe | yes |
   | 4 | 1280×720 | valid,safe | yes |
   | 5 | 1280×960 | valid,safe | yes |
   | 6 | 1344×756 | valid,safe | yes |
   | 7 | 1344×1008 | valid,safe | yes |
   | 8 | 1600×900 | valid,safe | yes |
   | 9 | 1600×1200 | valid,safe | yes |
   | 10 | **1920×1080** | valid,safe,**default,native** (`0x02000007`) | yes |
   | 11 | 640×480 | valid | no |

2. **The 1× 1920×1080 target exists and is the display's `default`+`native`
   mode** (modeID 10). It satisfies the `[[Framebuffer-pixel contract]]`:
   `pixelWidth=1920` (on the vision distribution) **and** `width=1920 pt` (layout
   parity with Linux/Windows). The clone boots into modeID 3 (1024×768) — the
   golden's WindowServer-saved mode — which is why `screen size` reported
   1024×768 pre-switch (reproducing ADR-0013's finding).

   *Bearing on the deferred HiDPI question (plan-k1 D4):* VF offers **no** Retina
   mode here, so the "1920×1080 only as 3840-px Retina" risk does **not**
   materialise and the strategic HiDPI question is **not forced**. A future
   `hidpi-vision` grove would first have to make VF advertise a HiDPI mode at all.

3. **The switch primitive must be a persistent transaction, not the bare call.**
   `CGDisplaySetDisplayMode(display, modeID10, nil)` returns `kCGErrorSuccess` and
   the active mode reads 1920×1080 *within the calling process* — but it is
   `.forAppOnly`-scoped: a **separate** process run immediately afterward reads
   the active mode back at **1024×768**, and `screen size` stays 1024×768. The
   change reverts the instant the setting process exits. Switching instead via
   `CGBeginDisplayConfiguration` → `CGConfigureDisplayWithDisplayMode` →
   `CGCompleteDisplayConfiguration(_, .forSession)` (CGError 0) **persists past
   process exit**: a separate reader process reads 1920×1080, and **`screen size`
   reports 1920×1080** (was 1024×768) on a fresh RFB connection. So VF resizes the
   host-side framebuffer to follow the guest's chosen mode — but only for a
   change that outlives the helper. `.forSession` is the right scope (per-instance
   runtime, not written to prefs / not baked); `.permanently` is unnecessary.

4. **Brief async settle transient after the switch.** The first `/exec` issued
   immediately after `CGCompleteDisplayConfiguration` stalled to the agent's 30 s
   exec timeout (while still returning its output); subsequent calls completed in
   ~0.1 s and agent health stayed reachable with accessibility granted. The
   framebuffer reconfiguration is asynchronous and briefly stalls the guest, then
   fully settles. **Build-leaf implication:** the synchronous switch in
   `TartRunner::start` should tolerate this settle window (the switch `/exec` may
   itself run long, or a short post-switch readiness re-check may be wanted) — it
   is not an error.

No macOS golden regeneration was required; the entire switch ran over the agent's
existing `/upload`+`/exec` surface against a stock clone. The probe is committed
at `provisioner/helpers/probe-display-modes.swift`.

## Implementation (2026-06-24, `build-resolution-switch-k3`)

The runtime switch ships:

- **Helper** `provisioner/helpers/set-display-mode.swift` — the production trim
  of the spike probe, parameterized by target px (argv `<w> <h>`). It selects
  the 1× mode (`pixelWidth==w && width==w && …`), switches via the persistent
  `.forSession` configuration transaction (finding 3), confirms the active mode
  reads the target, and exits non-zero with a one-line stderr reason on bad
  args / no-matching-mode / transaction failure.
- **Plumbing** `cli-rs/crates/testanyware-vm/src/display.rs` — `include_str!`-
  embeds the helper, host-compiles it with `swiftc`, uploads it over the agent
  `/upload`, and `/exec`s it (60 s timeout to absorb the finding-4 settle
  transient). `parse_target` derives the px target from the same resolved
  `--display` value that feeds `tart set` (unit suffix ignored — px == pt at 1×).
- **Wiring** in `VmLifecycle::start_tart` (lifecycle.rs), **after** the
  agent-readiness wait and synchronously before `vm start` returns, **macOS
  guests only**. Best-effort: a missing `swiftc` / compile / upload / exec
  failure warns and leaves the VM started (rides the optionally-degraded agent
  contract); agent-unreachable skips the switch entirely (no switch attempted).

**Live result:** `vm start --platform macos` with no `--display` against a fresh
`testanyware-golden-macos-tahoe` clone → the start log shows the in-guest switch
to modeID 10, and `testanyware screen size` reports **1920×1080** (was 1024×768).
The switch executes **inside the guest** via the agent — the host display is
never touched. No golden regeneration was required.
