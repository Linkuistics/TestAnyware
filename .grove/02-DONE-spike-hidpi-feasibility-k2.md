# spike-hidpi-feasibility-k2

**Kind:** work (feasibility spike)

## Goal

Answer the grove's load-bearing unknown: **can a *headless* macOS
Virtualization.framework guest be made to advertise AND select a 2× backing-scale
(Retina) display mode, and what does the host-side RFB framebuffer become when it
does?** This is the gate ADR-0014's spike finding 2 named: VF currently advertises
**zero** Retina modes, so this grove "would first have to make VF advertise a HiDPI
mode at all." Until this is CONFIRMED or REFUTED, the downstream design (target
shape, vision disposition) cannot be grilled.

## Context

Read first: root `BRIEF.md`, `CONTEXT.md` `[[Framebuffer-pixel contract]]`,
**ADR-0014** (especially Verification + Implementation), `01-plan-k1.md`'s
Decisions log (D1–D3 — the driver, the gate, this spike's scope).

The driver is **test realism** (plan-k1 D1): apps under test should render at 2×
like real Macs. The vision pipeline is a downstream consumer kept on its
1920×1080-px distribution (a constraint, not the goal). The likely-but-unconfirmed
design is *render 2× → 3840×2160 px → downsample 2:1 → vision sees native
1920×1080 px* — but that presumes VF reports the framebuffer at **px**, which only
this spike can confirm.

The existing `provisioner/helpers/probe-display-modes.swift` and
`set-display-mode.swift` enumerate `CGDisplayCopyAllDisplayModes` and switch via a
`.forSession` configuration transaction. **That machinery selects from advertised
modes — it does not create one.** The 12 advertised modes are all scale 1.0, so the
new problem is *making a 2× mode appear in that list*, a different problem class.

## Method

Mirror ADR-0014's spike exactly: a host-compiled Swift probe `/upload`+`/exec`'d
into a **fresh clone** of the live `testanyware-golden-macos-tahoe`, started via
the Rust CLI (`tart set --display 1920x1080px` default). NO golden regeneration.
Measure the host-side framebuffer with `testanyware screen size` (the negotiated
RFB ServerInit — the contract) from a *separate* connection after each change.
See the VM-from-harness memory notes (tart list state column, COPYFILE_DISABLE,
no backgrounded SSH).

## Questions the spike must answer (name them in the output)

**Scope (plan-k1 D3): guest-side first, then characterize host-side. Honor
ADR-0014's "no tart change, no golden regeneration" for the guest-side attempt.**

1. **Guest-side: can a 2× mode be made to appear and be selected?** Investigate,
   try the most viable, cite a primary source per mechanism claim:
   - a **display-override plist** under `/Library/Displays/Contents/Resources/Overrides/`
     (`DisplayVendorID-…/DisplayProductID-…` with HiDPI/`scale-resolutions` keys) —
     does writing it (agent `/exec`) + a WindowServer restart / re-login make
     `CGDisplayCopyAllDisplayModes` advertise a 2× mode? Is the path SIP-writable in
     the guest? How is the VF virtual display's vendor/product ID obtained?
   - **private CGS HiDPI-enable** APIs (e.g. `CGSConfigureDisplayEnabledForHiDPI`,
     the `CGSGet/SetDisplayModeDescription` family) — fragile, private; cite the
     tool/source you base any call on.
2. **What does `screen size` (RFB framebuffer) become under a selected 2× mode?**
   px (3840×2160) or pt (1920×1080)? *This single measurement decides whether the
   downscale design applies* — capture it explicitly.
3. **Does it persist past the helper process exiting** (the `.forAppOnly` vs
   `.forSession` lesson, ADR-0014 finding 3) and **work headless** (no attached
   display session)? Any settle/transient like ADR-0014 finding 4?
4. **Host-side fallback (characterize only if guest-side is REFUTED):** does the VF
   display config expose a Retina-class `pixelsPerInch` (`VZMacGraphicsDisplayConfiguration`)?
   Does **tart** surface it, or would it mean forking/bypassing tart? Don't build it
   — establish enough (with sources) that the follow-on planning leaf has a real
   fork, not a dead end.

## Done when

- A **verdict** (CONFIRMED / REFUTED for the guest-side path) backed by the
  measurements above, in the same shape as ADR-0014's Verification section.
- The **framebuffer-under-2× measurement** (Q2) is recorded explicitly.
- If guest-side REFUTED: the host-side option is **characterized** (Q4) with sources.
- Findings written up so they can retire into an **ADR amendment/new ADR** and a
  `docs/research/` note if the mechanism survey is substantial; the probe committed
  under `provisioner/helpers/`.
- The follow-on **target-shape + vision-disposition planning leaf** is grown
  (`leaf-add .`) with the spike's answer in hand (do this as the spike retires).

## Notes

Citation discipline (driving.md): a mechanism claim without a primary source
(Apple header, a real tool's source, a documented issue) is mood, not evidence —
those citations *become* the ADR's rationale. Record "no source found" as a finding
too. Keep the spike low-context: the goal is the gate, not the build.
