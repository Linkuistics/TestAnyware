# spike-display-modes-k2

**Kind:** work (spike — the **hard gate** for this grove)

## Goal

Empirically resolve **ADR-0014's load-bearing unknown #1**: after
`tart set --display 1920x1080px`, does a headless macOS VF guest advertise a
**selectable 1920×1080 CoreGraphics display mode at backing scale 1.0** that
`CGDisplaySetDisplayMode` will actually switch to? Answer with **real data from a
live golden**, not a guess. Everything downstream is gated on this.

## Context

- The mechanism is decided (ADR-0014): upload a host-compiled CoreGraphics
  helper through the agent's `/upload`+`/exec` surface and exec it. This spike
  de-risks the one unverified premise before the build leaf relies on it.
- **Target (plan-k1 D4):** 1920×1080 **px @1×** — `pixelWidth=1920` (vision
  distribution) **and** `width=1920 pt` (layout parity). See
  `[[Framebuffer-pixel contract]]`. **This is why pt+px+scale must be reported
  per mode** — a Retina-only 1920×1080 (3840 px) would *not* satisfy the target.
- No CoreGraphics display-mode code exists in the repo yet (verified) — the probe
  is net-new.
- Channel: the **agent HTTP client** (`testanyware-agent-client`: `client.upload`,
  `client.exec`), NOT the golden-creation SSH path. `set-wallpaper.swift` +
  `golden.rs::provision_wallpaper` is the host-compile→upload→exec **pattern**
  template (but it runs over SSH at golden time; here it's HTTP at `vm start`).
- Memory: use `tart list` **state** column (not `tart ip`) to confirm running;
  golden is kept built; clone+start is cheap.

## Deliverables

1. **Probe** `provisioner/helpers/probe-display-modes.swift` (kept committed):
   `CGDisplayCopyAllDisplayModes` for the main display → print **per mode**:
   `width`(pt), `height`(pt), `pixelWidth`, `pixelHeight`, derived scale,
   refresh, IOFlags. Then attempt `CGDisplaySetDisplayMode` to the mode where
   `pixelWidth==1920 && pixelHeight==1080 && width==1920` (the 1× target);
   re-read the active mode and report whether the switch took. Host-compile with
   `swiftc`, upload via `client.upload`, exec via `client.exec`.
2. Run it against a **fresh clone** of the macOS golden (after `vm start`, agent
   ready). Capture the **full mode list** and the switch result.
3. Append a dated **`## Verification`** section to
   `docs/adr/0014-macos-guest-resolution-runtime-switch.md`: modes advertised,
   whether a 1× 1920×1080 mode exists & is selectable, the
   `CGDisplaySetDisplayMode` result, and a **CONFIRMED / REFUTED** verdict.
   (Mirrors ADR-0013's verification-section precedent.)

## On REFUTE (no selectable 1× 1920×1080 mode)

Do **not** silently proceed. `grove-llm leaf-insert 03-build-resolution-switch-k3
fallback-replan` to sequence a replan leaf **ahead** of the build, and note the
known fallbacks in its brief: a **private CGS mode-creation API** (synthesize the
mode), or **golden-bake** (ADR-0014 rejected it on correctness grounds — revisit
only if forced). If VF offers 1920×1080 only as a **Retina/3840-px** mode (no 1×),
that forces the deferred HiDPI question (plan-k1 D4) — surface it, don't decide it
solo. Mark the build leaf's brief void/blocked pending the replan.

## Done when

- The probe is committed and has been **run against a live golden**.
- ADR-0014 carries a Verification section with a CONFIRMED/REFUTED verdict backed
  by the captured mode list.
- On CONFIRMED: the build leaf (`k3`) is unblocked as-is. On REFUTED: the replan
  leaf is inserted and `k3` marked blocked.

## Notes
