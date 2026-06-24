# hidpi-vision — brief

## Goal

Drive macOS guests at HiDPI/Retina (**2× backing scale**) so apps *under test*
render the way real Mac users see them — font smoothing, @2x asset selection,
hairline/sub-pixel rendering — instead of the 1× LoDPI mode almost no real Mac
uses. The vision pipeline is a downstream **consumer**; keeping it on its
1920×1080-px distribution is a *constraint to satisfy*, not the goal (plan-k1 D1).

## Done when

Feasibility is settled (**ADR-0015**) and the **build design** is settled
(**ADR-0016**, target-shape-and-vision-disposition-k3): a **scale-aware RFB
surface** (logical 1920×1080 over a physical 3840×2160 wire) shipped as an
**opt-in** over shipped tart's host-scale `pt` path; the *deterministic* host-side
mechanism is deferred. The grove is **done** when:

- `vm start --display 1920x1080@2x` on a Retina host brings a macOS guest up at 2×
  backing scale (physical 3840×2160 confirmed via `screen size`), with ADR-0014's
  1× switch suppressed — empirically confirmed (`confirm-hidpi-pt-path-on-retina-host`);
- the scale-aware `RfbConnection` presents a logical 1920×1080 surface to every
  consumer (exact 2:1 downsample on reads, ×2 pointer on writes, scale
  auto-detected, 1×=no-op), unit-tested (`build-scale-aware-rfb-connection`);
- the `@2x` opt-in is wired end-to-end (parse/translate, route to `pt`, suppress
  the 1× switch, host-scale warn, `--physical` capture/record); HiDPI renders an
  app under test at 2× while vision/clicks operate in logical 1920×1080
  (`build-hidpi-optin-and-wiring`);
- vision accuracy on the downsampled-2× frame is **measured** against the 1×
  baseline (`verify-vision-on-downsampled-2x`) — pass blesses vision-on-HiDPI;
  material fail documents HiDPI as realism/viewer-only + a retraining workstream.

**Out of scope (deferred, not a live leaf):** the deterministic tart-fork /
custom-VF mechanism for headless / 1×-host / CI HiDPI — ADR-0016 "Deferred",
triggered by real demand off a Retina host.

## Decomposition

Children are ordered by dependency — feasibility gated the design, which gates the
build; within the build, k4 (mechanism confirm) gates the end-to-end wiring.

1. `plan-k1` — initial design grilling: established the driver and the feasibility
   gate, grew the tree. (DONE)
2. `spike-hidpi-feasibility-k2` — the load-bearing feasibility gate (DONE). Verdict
   in **ADR-0015**: guest-side REFUTED, host-side viable via a tart fork / custom
   VF harness; the framebuffer is reported in px (3840×2160), so the 2:1-downscale
   design applies.
3. `target-shape-and-vision-disposition-k3` — planning (DONE). Settled the build
   design in **ADR-0016**: minimal opt-in over the `pt` path, scale-aware logical
   RFB surface, `--display WxH@2x`, vision gated on a verify leaf; deferred the
   deterministic fork. Grew k4–k7.
4. `confirm-hidpi-pt-path-on-retina-host-k4` — the load-bearing empirical de-risk
   (the doubt pass for ADR-0015's derived-not-measured `pt`→2× claim): on a Retina
   host, confirm `@2x` yields a 3840×2160 framebuffer and pin the `vm start`
   sequencing (does the guest need a switch to *select* the Retina mode?).
5. `build-scale-aware-rfb-connection-k5` — the reusable core: logical surface,
   exact 2:1 downsample, ×2 pointer, scale auto-detect. Mechanism-independent,
   unit-testable on any host.
6. `build-hidpi-optin-and-wiring-k6` — `@2x` parse/translate, route to `pt`,
   suppress ADR-0014's 1× switch, host-scale warn, `--physical` capture/record;
   wires k5 into `vm start` end-to-end.
7. `verify-vision-on-downsampled-2x-k7` — measure OCR + window-detection accuracy
   on downsampled-2× vs the native-1× baseline; bless or document the disposition.

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
