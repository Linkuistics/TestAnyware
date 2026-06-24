# build-resolution-switch-k3

**Kind:** work

**Presupposes `spike-display-modes-k2` CONFIRMED** that a selectable 1×
1920×1080 CoreGraphics mode exists. If the spike REFUTED, this leaf is **blocked**
pending the inserted fallback-replan leaf — do not build on a false premise.

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
  calls `CGDisplaySetDisplayMode`. Reuses the spike's already-verified CG calls.
- **Embed + host-compile**: mirror `golden.rs::provision_wallpaper` /
  `SET_WALLPAPER_SWIFT` — `include_str!` the helper, `swiftc -o` on the host.
- **Delivery (D3 default)**: **per-start upload** via `client.upload`, exec via
  `client.exec` (NOT SSH). Native `/set-display` endpoint reconsidered only if
  per-start proves a problem.
- **Wiring (D3 default)**: in `TartRunner::start` (`testanyware-vm/src/tart.rs`),
  **after** the agent-readiness wait, **synchronously before returning success**
  — so consumers waiting on `vm start` are gated by construction (the transient
  needs no separate handling).
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
