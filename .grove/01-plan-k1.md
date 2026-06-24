# plan-k1

**Kind:** planning

## Goal

Establish the design for the `hidpi-vision` grove: decide *what success looks
like* for driving macOS guests at HiDPI/Retina (2× backing scale), how that
collides with the 1920×1080-px vision pipeline, and what the first concrete
leaves are. Grow the tree — likely a feasibility spike (can VF advertise a 2×
mode at all?) plus downstream build/planning leaves.

## Context

`hidpi-vision` is the grove named in advance by `[[Framebuffer-pixel contract]]`
(CONTEXT.md) and ADR-0014's spike finding 2. It inherits a sharp fork:

- The glossary *defers* HiDPI to "a future dedicated grove that would rework the
  vision pipeline" — but also **debunks the two obvious motivations**: the 1×
  1920×1080 mode already gives vision-distribution parity (`pixelWidth=1920`) AND
  layout parity (`width=1920pt`). "Same window sizes needs 2× the pixels" is
  called a myth.
- ADR-0014's spike found VF advertises **12 display modes, all at backing scale
  1.0 — zero Retina modes** for the headless guest, and concluded the strategic
  HiDPI question is "not forced… a future grove would first have to make VF
  advertise a HiDPI mode at all."

So the enabling mechanism is an unconfirmed unknown, and the motivation had to be
re-established from scratch (see D1).

## Done when

- The driver, the target framebuffer shape, and the vision-pipeline disposition
  are settled with the user (shared understanding).
- The tree is grown with concrete next leaves (feasibility spike + downstream).
- Any durable decision is captured (ADR / glossary update).

## Decisions (running log)

**D1 — Driver is test realism.** The grove exists so apps *under test* render at
the 2× backing scale real Macs use (font smoothing, @2x asset selection,
hairline/sub-pixel rendering); 1× LoDPI is a config almost no real Mac uses and
may hide or invent rendering bugs. Explicitly **not** primarily a vision/OCR
accuracy play — the glossary already debunks the vision-distribution rationale.
Consequence: the *app under test* is the thing that must render at 2×; the vision
pipeline is the test harness's eyes, a downstream consumer of whatever framebuffer
results — which sets up D2 (what do we feed vision?).

**D2 — Gate on feasibility before fixing vision scope.** Whether to keep vision
on its 1920×1080-px distribution (downscale path) or rework the models is **not
decided yet** — it is gated on a feasibility spike. The load-bearing unknown
(ADR-0014 spike finding 2): VF advertises zero Retina modes for the headless
guest. Until we know (a) whether VF *can* be made to advertise/select a 2×
backing-scale mode headless, and (b) what framebuffer shape results, the
downstream design (target shape, vision disposition) cannot be grilled
productively. Consequence: the first concrete leaf is a **feasibility spike**;
the target-shape + vision-disposition planning is deferred until it returns.

**The core tension the spike must illuminate (for the downstream decision).** At
2× backing scale, `framebuffer_px = logical_pt × 2`, so the two axes can't both
stay fixed:
- Keep framebuffer 1920×1080 px → logical collapses to 960×540 pt (cramped,
  unrealistic — defeats the realism driver in *layout*).
- Keep logical 1920×1080 pt (realistic) → framebuffer 3840×2160 px (off the
  vision distribution).
- Escape hatch: render 2× → 3840×2160 px → **downsample exactly 2:1** → 1920×1080
  px = the vision pipeline's native geometry. App gets Retina; vision gets its
  trained input; pointer events map by a clean ×2. This is the path D2 will
  likely converge on *if* the spike confirms feasibility — but only the spike can
  tell us whether VF reports the framebuffer at px (3840) or pt (1920), which
  decides whether the downscale even applies.

**D3 — Spike scope: guest-side first, characterize host-side, report the
framebuffer.** The spike's central job is *making VF advertise* a selectable 2×
mode (the existing `CGDisplayCopyAllDisplayModes` shows none — every mode is scale
1.0). Mechanism search honors ADR-0014's principle: try **guest-side** first
(`/Library/Displays` HiDPI-override plist; private CGS HiDPI-enable) via the agent
`/exec` channel, no tart/golden change. If guest-side is **refuted**, the spike
must still *characterize* the **host-side** option (does VF/tart expose a
Retina-class `pixelsPerInch`? at what cost — fork/bypass tart?) so the follow-on
planning leaf gets a real fork, not a dead end. Either way the spike reports **what
the host-side RFB framebuffer (`screen size`) becomes under a 2× mode** — px
(3840×2160) or pt (1920×1080) — because that single measurement decides whether
the downscale path (D2) is the right design. Vision-impact measurement is
explicitly a *later* leaf, not the spike's job.

→ Grew the tree: `02-spike-hidpi-feasibility-k2` (this session). Target-shape +
vision-disposition planning and any build/verify leaves are deferred — grown
lazily once the spike returns (constraint 4).

## Notes
