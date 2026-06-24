# macos-guest-resolution — brief

## Goal

Implement the **runtime, agent-mediated guest-side display-mode switch** that
[ADR-0014](../docs/adr/0014-macos-guest-resolution-runtime-switch.md) decided
and deferred. At `vm start` on macOS, after the in-VM agent is ready, force the
guest's framebuffer to the resolved resolution (the **1920×1080 px** default
from [ADR-0013](../docs/adr/0013-default-guest-display-resolution.md), or the
user's `--display`) by uploading a host-compiled CoreGraphics helper through the
agent's existing `/upload` + `/exec` surface and exec'ing it.

The mechanism is **already decided** (ADR-0014). This grove's job is to
**de-risk the load-bearing unknown first, then build it** — not to re-open the
design.

## Done when

- A macOS-on-tart guest reports **1920×1080 px** from `screen size` (the
  negotiated RFB framebuffer — the contract) after `vm start` with no
  `--display`, where today it reports 1024×768.
- The mechanism requires **no macOS golden regeneration** (ADR-0014's constraint).
- Linux/QEMU paths are untouched (already correct per ADR-0013 verification).

## Decomposition

Settled by `plan-k1` (see its running log for D1–D4):

1. **`spike-display-modes-k2`** — the **hard gate**. Clone+start the live macOS
   golden; upload a probe via the agent `/upload`+`/exec`; dump every advertised
   CoreGraphics mode with **pt + px + scale**; attempt the switch to the 1×
   1920×1080 mode. Append a **Verification** section to ADR-0014; keep the probe
   committed. On REFUTE, `leaf-insert` a fallback-replan leaf (private CGS API /
   golden-bake) ahead of the build leaf.
2. **`build-resolution-switch-k3`** — presupposes spike CONFIRMED. Trim the probe
   into a real helper **parameterized by target px**, selecting the **1× mode**;
   embed + host-compile it (mirror `set-wallpaper.swift`); wire into
   `TartRunner::start` **synchronously after agent readiness**, riding the
   existing **optionally-degraded** contract (no agent ⇒ warn, leave 1024×768,
   don't fail `vm start`); live-verify `screen size == 1920×1080`.

**Target (D4):** 1920×1080 **px @1×** (LoDPI) — on the vision distribution *and*
layout-parity with Linux/Windows. **HiDPI/Retina is explicitly out of scope** —
a future `hidpi-vision` grove flips the (parameterized) target + reworks vision.

## Pointers

- **ADR-0014** — the decision being implemented; names the load-bearing unknown
  (does VF advertise a *selectable* 1920×1080 CG display mode for a headless
  guest after `tart set --display 1920x1080px`?) — de-risk with a spike first.
- **ADR-0013** — the 1920×1080-px default and the tart pt/px hint asymmetry.
- Glossary: `[[Guest-controlled resolution]]`, `[[Agent a11y surface]]` (the
  `/exec`+`/upload` non-a11y surface), `[[Host-side framebuffer]]`.
- Code touchpoints (verified 2026-06-24):
  - `cli-rs/crates/testanyware-vm/src/tart.rs` — `set_display` (unconditional at
    start, `DEFAULT_DISPLAY = "1920x1080px"`), `poll_ip` (Option, benign
    degradation), `TartRunner::start` sequence.
  - `cli-rs/crates/testanyware-vm/src/lifecycle.rs` — agent-readiness gate;
    `agent_unreachable` benign-degradation path.
  - `cli-rs/crates/testanyware-agent-client/src/lib.rs` — `client.exec`,
    `client.upload` (the runtime channel; NOT the golden-creation SSH channel).
  - `provisioner/helpers/set-wallpaper.swift` + `golden.rs` `provision_wallpaper`
    — the host-compile→upload→exec **pattern template** (but over SSH).
  - `cli-rs/crates/testanyware-cli/src/commands/screen.rs` — `screen size`, the
    verification contract.

## Notes

- No CoreGraphics display-mode code exists anywhere in the repo yet (verified) —
  the helper is net-new.
- Spike is runnable in this grove: host is macOS (darwin), the macOS golden is
  kept built.
