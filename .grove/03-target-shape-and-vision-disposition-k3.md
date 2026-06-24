# target-shape-and-vision-disposition-k3

**Kind:** planning

## Goal

With feasibility settled (ADR-0015: HiDPI is a **host-side VF display-config**
concern, guest-side REFUTED), grill the **downstream design** the spike deferred
(plan-k1 D2): the target framebuffer shape and the vision-pipeline disposition
for the HiDPI path, and grow the build leaves. Land the agreement (likely a PRD
and/or an ADR) and decompose into concrete build/verify work.

## Context

Read first: root `BRIEF.md`, `CONTEXT.md` `[[Framebuffer-pixel contract]]` +
`[[Guest-controlled resolution]]`, **ADR-0015** (the verdict — start here),
`docs/research/hidpi-enable-mechanisms.md` (mechanism survey + measurements),
ADR-0013 + ADR-0014 (the 1× default + runtime switch this opt-in sits beside).

What the spike fixed (don't re-litigate):
- HiDPI is reached **host-side** by injecting an explicit high `pixelsPerInch`
  into `VZMacGraphicsDisplayConfiguration`. tart-as-shipped can't do it
  deterministically (hardcodes ppi 72 headless; inherits host scale on `pt`).
- The **downscale design applies**: a 2× config of logical 1920×1080 ⇒ RFB
  `screen size` reports **px** 3840×2160, so *render 2× → downsample exactly
  2:1 → vision's native 1920×1080 px; clicks ×2* (plan-k1 D2) is the right shape.
- The 1× default (ADR-0013/0014) **stands**; HiDPI is an opt-in alternative
  disposition, not a replacement.

## Open questions to grill (the design fork ADR-0015 left open)

1. **Mechanism: fork tart vs custom VF harness.** Patch tart to expose a ppi /
   `@2x` display option, vs a parallel `s-u/macosvm`-style VF host process. Cost,
   maintenance, how it rides the existing `tart run --vnc-experimental` plumbing
   (`tart.rs` `spawn_detached`). Which keeps the backend swap honest (ADR-0010)?
2. **Where the 2:1 downsample lives.** Host RFB stage (downsample 3840×2160 →
   1920×1080 before vision), a new pipeline stage, or the embedded viewer path?
   Cost per frame (3840×2160 is ~4× the bytes — ADR-0013 already flagged frame
   size); does `screen capture`/`screen record` see px or downsampled?
3. **Pointer-event ×2 mapping.** Vision targets in 1920×1080 px → guest events in
   3840×2160 px. Where does the ×2 live (input layer), and does it interact with
   the agent's element-based acting (which is resolution-independent)?
4. **Opt-in surface.** A `--display 1920x1080@2x` style flag? a run mode? How
   does `vm start` sequence the host-side 2× config (VM-construction time, unlike
   ADR-0014's post-boot agent switch)?
5. **Vision disposition (the deferred D2 half).** Confirm the downsample keeps
   vision on-distribution (a *later verify leaf* measures accuracy — this leaf
   only fixes the design); is any retraining implied, or does 2:1 downsample of a
   2× render land close enough to the 1× distribution? Name the measurement leaf.

## Done when

- The mechanism choice (Q1) and the downsample/pointer disposition (Q2–Q4) are
  settled with the user; the vision-disposition design (Q5) is fixed with its
  verify leaf named.
- Durable decisions captured (PRD at the agreement point and/or an ADR amending
  ADR-0015's "build design deferred"). The grove root `BRIEF.md` "Done when" is
  tightened from ADR-0015's verdict into a concrete success bar.
- The tree is grown with concrete build + verify leaves.

## Notes

This is a **planning** task — open with grilling (one question at a time,
recommend an answer per step; see grilling.md / driving.md). Keep ADR-0015 as the
settled floor; the spike already did the feasibility work, so this leaf is design,
not re-investigation.
