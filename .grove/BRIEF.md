# hidpi-vision — brief

## Goal

Drive macOS guests at HiDPI/Retina (**2× backing scale**) so apps *under test*
render the way real Mac users see them — font smoothing, @2x asset selection,
hairline/sub-pixel rendering — instead of the 1× LoDPI mode almost no real Mac
uses. The vision pipeline is a downstream **consumer**; keeping it on its
1920×1080-px distribution is a *constraint to satisfy*, not the goal (plan-k1 D1).

## Done when

Feasibility is now known (**ADR-0015**, spike-hidpi-feasibility-k2): HiDPI is a
**host-side VF display-config** concern — guest-side REFUTED, reached by injecting
an explicit high `pixelsPerInch` (tart fork / custom VF harness). The provisional
success bar, to be tightened by `target-shape-and-vision-disposition-k3`:

- a macOS guest can be brought up rendering at **2× backing scale** (3840×2160-px
  framebuffer for logical 1920×1080), via the host-side mechanism;
- the host-side RFB framebuffer feeds vision its native **1920×1080 px** by an
  exact **2:1 downsample**, with pointer events mapped ×2 — and a verify leaf
  confirms vision stays on-distribution;
- HiDPI is an **opt-in** disposition that leaves the 1× default (ADR-0013/0014)
  intact; the mechanism + disposition are recorded in ADR(s).

## Decomposition

Children are ordered by dependency — feasibility gates everything downstream.

1. `01-plan-k1` — initial design grilling: established the driver and the
   feasibility gate, grew the tree. (this session)
2. `02-spike-hidpi-feasibility-k2` — the load-bearing gate (DONE). Verdict in
   **ADR-0015**: guest-side REFUTED, host-side viable via a tart fork / custom VF
   harness; the framebuffer is reported in px (3840×2160), so the 2:1-downscale
   design applies.
3. `03-target-shape-and-vision-disposition-k3` — planning: grill the build design
   the spike deferred (mechanism fork, downsample placement, pointer ×2, opt-in
   surface, vision disposition), land a PRD/ADR, grow build + verify leaves.

## Pointers

- `CONTEXT.md` `[[Framebuffer-pixel contract]]`, `[[Guest-controlled resolution]]`
  — names this grove; defines the px-vs-pt distinction this grove turns on.
- **ADR-0013** — TestAnyware-owned default 1920×1080-**px** resolution; calls
  retraining the vision models "a separate workstream".
- **ADR-0014** — macOS runtime resolution switch (the just-completed grove). Its
  spike finding 2 found VF advertises **zero** Retina modes and named
  `hidpi-vision` as the grove that "would first have to make VF advertise a HiDPI
  mode at all". Read its Verification + Implementation sections — the spike here
  extends that machinery.
- `provisioner/helpers/{set-display-mode,probe-display-modes}.swift` — the CG
  mode-enumeration + `.forSession` configuration-transaction pattern to reuse.
- `cli-rs/crates/testanyware-vm/src/{display.rs,tart.rs,lifecycle.rs}` — the
  runtime-switch plumbing (`vm start` macOS path, agent `/upload`+`/exec`).

## Notes

The two textbook motivations for HiDPI are **already debunked** by the
framebuffer-pixel contract — vision-distribution parity AND layout parity both
hold at 1×. The *only* residual driver is test realism (D1); do not reintroduce
the debunked rationales.

**Core tension** (the heart of the downstream design): at 2×,
`framebuffer_px = logical_pt × 2`, so 1920×1080 px and 1920×1080 pt cannot both
hold. Likely resolution: render at 2× (3840×2160 px) → **downsample exactly 2:1**
→ feed vision its native 1920×1080 px; clicks map by a clean ×2. But this is
*provisional* — it presumes VF reports the framebuffer at px (3840), which only
the spike can confirm.
