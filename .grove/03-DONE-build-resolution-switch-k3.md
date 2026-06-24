# build-resolution-switch-k3

**Kind:** work

**Spike `spike-display-modes-k2` CONFIRMED** (2026-06-24): a selectable 1×
1920×1080 mode exists (modeID 10, `default`+`native`), and a persistent
config transaction switches the RFB framebuffer to it (`screen size` →
1920×1080). This leaf is **unblocked**. See ADR-0014's Verification section for
the full mode list and the two material refinements baked into the decisions
below (transaction not bare call; post-switch settle).

## Goal

Make `vm start` on macOS force the guest framebuffer to the resolved resolution
(default **1920×1080 px @1×**, or `--display`) by uploading a host-compiled
CoreGraphics helper through the agent and exec'ing it — **no golden
regeneration**. Implements ADR-0014.

## Context & decisions (from plan-k1)

- **Helper**: trim the spike's `probe-display-modes.swift` into a real switch
  helper at `provisioner/helpers/` — **parameterized by target px** (argv), it
  selects the mode where `pixelWidth==target_w && pixelHeight==target_h &&
  width==target_w` (the **1× mode** — see `[[Framebuffer-pixel contract]]`) and
  commits it via a **persistent configuration transaction**
  (`CGBeginDisplayConfiguration` → `CGConfigureDisplayWithDisplayMode` →
  `CGCompleteDisplayConfiguration(_, .forSession)`) — **not** the bare
  `CGDisplaySetDisplayMode(_, _, nil)`, which the spike found is `.forAppOnly`-
  scoped and **reverts the moment the helper exits** (ADR-0014 Verification
  finding 3). The spike's transaction calls are the already-verified seed.
  Exit non-zero with a clear message if no matching 1× mode is found, so the
  caller can warn.
- **Embed + host-compile**: mirror `golden.rs::provision_wallpaper` /
  `SET_WALLPAPER_SWIFT` — `include_str!` the helper, `swiftc -o` on the host.
- **Delivery (D3 default)**: **per-start upload** via `client.upload`, exec via
  `client.exec` (NOT SSH). Native `/set-display` endpoint reconsidered only if
  per-start proves a problem.
- **Wiring (D3 default)**: in `TartRunner::start` (`testanyware-vm/src/tart.rs`),
  **after** the agent-readiness wait, **synchronously before returning success**
  — so consumers waiting on `vm start` are gated by construction (the transient
  needs no separate handling). Tolerate the **post-switch settle transient**
  (ADR-0014 Verification finding 4): the switch `/exec` may itself run several
  seconds while VF reconfigures the framebuffer — give it generous timeout and
  treat a brief stall as normal, not an error.
- **Failure mode (D3 default)**: ride the existing **optionally-degraded** agent
  contract (`poll_ip`→`Option`, `agent_unreachable`). Agent not ready ⇒ **warn +
  leave 1024×768, do NOT fail `vm start`**. Resolution is best-effort, like the
  agent endpoint itself.
- **`--display` footgun (out of scope)**: this grove targets the **default**
  path. The pre-existing macOS pt/px footgun on explicit `--display` (ADR-0013)
  is **not** fixed here; document the interaction (the resolved value now feeds
  both `tart set` and the helper — derive the helper's px target consistently).
- **Linux/QEMU untouched** — already correct (ADR-0013 verification).

## Deliverables

- The helper source (committed) + host-compile/embed plumbing.
- `vm start` macOS path performs the synchronous, agent-mediated switch with
  degraded-fallback handling.
- Unit tests where they fit (mode-selection logic; the wiring is
  integration-shaped).
- **Live verify**: `vm start` (no `--display`) against a fresh macOS clone →
  `screen size` reports **1920×1080** (today: 1024×768). Record the result in
  ADR-0013/0014 verification notes as appropriate.

## Done when

- A macOS-on-tart guest reports **1920×1080 px** from `screen size` after
  `vm start` with no `--display`, verified live.
- No macOS golden regeneration was required.
- The degraded path (agent not ready) warns and leaves the VM started, not failed.

## Notes

- If this proves bigger than one focused session (e.g. helper + wiring +
  degradation + tests each substantial), `grove-llm leaf-decompose` it and do
  only the first child.
