# hidpi-vision — brief

## Goal

Drive macOS guests at HiDPI/Retina (**2× backing scale**) so apps *under test*
render the way real Mac users see them — font smoothing, @2x asset selection,
hairline/sub-pixel rendering — instead of the 1× LoDPI mode almost no real Mac
uses. The vision pipeline is a downstream **consumer**; keeping it on its
1920×1080-px distribution is a *constraint to satisfy*, not the goal (plan-k1 D1).

## Done when

*Deferred until feasibility is known (plan-k1 D2).* The success bar is set after
the feasibility spike returns. Provisionally: a macOS guest can be brought up
rendering at 2× backing scale, the host-side RFB framebuffer + vision pipeline
have a defined and working disposition, and the mechanism is recorded in an ADR.

## Decomposition

Children are ordered by dependency — feasibility gates everything downstream.

1. `01-plan-k1` — initial design grilling: established the driver and the
   feasibility gate, grew the tree. (this session)
2. `02-spike-hidpi-feasibility-k2` — the load-bearing gate: can VF be made to
   advertise + *select* a 2× mode for a **headless** guest, and what does the RFB
   framebuffer become? Guest-side first, characterize host-side (plan-k1 D3).
3. *(grown after the spike)* target-shape + vision-disposition planning, then
   build + verify — added lazily once the spike answers feasibility.

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
