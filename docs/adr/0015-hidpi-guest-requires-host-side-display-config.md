# 15. HiDPI/Retina on a macOS guest is a host-side VF display-config concern, not reachable guest-side

Date: 2026-06-25

## Status

Accepted (feasibility spike `spike-hidpi-feasibility-k2`, grove `hidpi-vision`).
This ADR records a **feasibility verdict** that gates the grove's downstream
design; the build is deferred to a follow-on planning leaf. It extends ADR-0014
(which deferred the HiDPI question — its spike finding 2 noted VF advertises
**zero** Retina modes and named `hidpi-vision` as "the grove that would first
have to make VF advertise a HiDPI mode at all").

## Context

The `hidpi-vision` grove's driver is **test realism** (plan-k1 D1): apps under
test should render at the **2× backing scale** real Macs use (font smoothing,
@2× asset selection, hairline/sub-pixel rendering), instead of the 1× LoDPI mode
almost no real Mac uses. The vision pipeline is a downstream consumer kept on its
1920×1080-px distribution (a constraint, not the goal).

The load-bearing unknown: **can a headless macOS VF guest be made to advertise
*and* select a 2× display mode, and what does the host-side RFB framebuffer
become?** ADR-0014 found VF advertises only scale-1.0 (LoDPI) modes. Per
plan-k1 D3 the spike tried **guest-side first** (honouring ADR-0014's "no tart
change, no golden regeneration"), then characterized the host-side option.

## Decision

**Treat HiDPI/Retina as a host-side Virtualization.framework display-configuration
concern. There is no guest-side mechanism; the downstream build must change the
host-side `VZMacGraphicsDisplayConfiguration` (a tart fork or a custom VF
harness).**

The spike establishes the invariant that forces this: a macOS guest's advertised
CoreGraphics modes — both their *sizes* and their *backing scale* — are derived
from the **host-side** VF display config. The guest's WindowServer only *selects*
from that host-defined menu ([[Guest-controlled resolution]]); nothing written
inside the guest reaches back into the VF config. So:

- **Guest-side is REFUTED.** The `/Library/Displays` override-plist mechanism had
  **zero effect** on the VF virtual display (measured: override + reboot → no new
  modes), because the panel is identity-less and, more fundamentally, an override
  cannot create a backing scale the host config never produced. The private
  CGS/SkyLight APIs can only *select* an advertised mode (modern Apple Silicon
  validates against the DCP-derived mode list and rejects fabricated ones), so
  they cannot inject a 2× mode either.
- **Host-side is the viable path** but tart-as-shipped cannot do it
  deterministically: tart hardcodes `pixelsPerInch: 72` (non-Retina) on the
  headless/`px` path, and on the `pt` path inherits the *host monitor's* backing
  scale — which is absent on a headless/1× host and varies per dev/CI machine. A
  deterministic 2× requires injecting an explicit high `pixelsPerInch`, which
  tart does not surface.

The full mechanism survey, citations, and measurements are in
`docs/research/hidpi-enable-mechanisms.md`.

## Considered options

- **Guest-side `/Library/Displays` HiDPI override plist** (per `one-key-hidpi`).
  *Rejected — measured no-op.* The VF virtual display reports CG vendor/model
  `0/0` with no `IODisplayPrefsKey`/`LegacyManufacturerID`, so the EDID-keyed
  override never binds; and even a bound override cannot manufacture a backing
  scale the host config didn't create. Matches the one external primary source
  that tried it on a virtual display (BetterDisplay #1747).
- **Guest-side private CGS/SkyLight mode injection.** *Rejected — wrong tool.*
  `CGSConfigureDisplayMode`/`SLConfigureDisplayWithDisplayMode` *select* an
  advertised mode; they don't create one, and Apple-Silicon validation rejects
  out-of-list modes (error 1000). A `CGVirtualDisplay` would add a *fake second*
  display, not make the app-under-test's real display Retina — it doesn't serve
  the realism driver. `CGSConfigureDisplayEnabledForHiDPI` has **no primary
  source** (likely confabulated).
- **tart `--display …pt` on a Retina host (no code change).** *Rejected as the
  product mechanism — viable only as an ad-hoc dev convenience.* It works
  *only* when the host's `NSScreen.main` is Retina; it is non-deterministic
  across hosts and fails headless/1×. Unsuitable for a reproducible test harness.
- **Fork tart / custom VF harness injecting an explicit ppi** (the `s-u/macosvm`
  `dpi` model, ~200+). *Selected direction* — the only mechanism that yields a
  deterministic 2× framebuffer independent of the host monitor. Cost (a tart
  fork or a parallel VF host process) is real but bounded; macosvm is an existence
  proof. Detailed design deferred to the follow-on planning leaf.

## Consequences

- The grove's **core tension holds and the downscale design applies.** Under a
  host-injected 2× config of logical 1920×1080, the RFB framebuffer
  (`screen size`) reports **px** = 3840×2160 (VF config dimensions are pixels;
  ADR-0014 confirmed `screen size` reports the px framebuffer). So the downstream
  path *render 2× → 3840×2160 px → downsample exactly 2:1 → feed vision its native
  1920×1080 px; clicks map by a clean ×2* (plan-k1 D2) is the right shape — its
  px-not-pt precondition is satisfied.
- The downstream build is **not** a `vm start`-layer change like ADR-0014's
  guest-side switch. It touches the **VM-construction layer** (how tart/VF builds
  the display device) — a heavier change than ADR-0014, with a tart-fork /
  custom-harness maintenance cost the planning leaf must weigh against the
  realism benefit.
- ADR-0013's 1× `1920x1080px` default and ADR-0014's 1× runtime switch **stand
  unchanged** as the LoDPI default. HiDPI is an *opt-in alternative display
  disposition*, not a replacement — the vision pipeline still consumes 1920×1080
  px (via the 2:1 downsample on the HiDPI path).
- **Open question for the follow-on planning leaf** (target-shape +
  vision-disposition): tart-fork vs custom VF harness; where the 2:1 downsample
  lives (host RFB stage vs a new pipeline stage); pointer-event ×2 mapping; and
  whether HiDPI is per-instance opt-in (a `--display …@2x` style flag) or a
  separate run mode. This ADR fixes only *that* the mechanism is host-side and
  *which* family is viable — not the build design.

## Verification (2026-06-25, `spike-hidpi-feasibility-k2`)

**Verdict: guest-side REFUTED; host-side viable via a tart fork / custom VF
harness.** Full method, measurements, and citations in
`docs/research/hidpi-enable-mechanisms.md`. Summary:

- **Identity-less panel:** `CGDisplayVendorNumber/ModelNumber == 0`; no
  `IODisplayPrefsKey`/`LegacyManufacturerID`; IORegistry `ProductName="Apple
  Virtual"`, native 1024×768.
- **Override plist (Mechanism 1) — REFUTED:** installed at both candidate vendor
  keyings + `DisplayResolutionEnabled` + full reboot (files & pref survived) →
  mode list **byte-identical**, still 4 modes all scale 1.0, `retinaModeCount=0`,
  `screen size` 1024×768. `/Library/Displays` was writable with `sudo` under SIP
  enabled.
- **tart `pt` path (Mechanism 3) — host-coupled:** on a 1× host
  (`NSScreen.main.backingScaleFactor == 1.0`), `--display 1920x1080pt` advertised
  12 modes up to 1920×1080 but **all scale 1.0** — proving the advertised
  *sizes* track the config's points while the *scale* tracks the host monitor.
- **Q2 (framebuffer under 2×):** reported in **px** by construction — a 2× config
  of logical 1920×1080 ⇒ `screen size` 3840×2160 — so the downscale design
  applies. (Not directly measured at 2× for want of a Retina host / fork; this is
  the one finding derived rather than measured, and is the first thing the
  follow-on build will confirm.)

The probe is committed at `provisioner/helpers/probe-hidpi-identity.swift`. No
macOS golden regeneration was required; the entire guest-side attempt ran over
the agent's existing `/upload`+`/exec` surface against a stock clone.
