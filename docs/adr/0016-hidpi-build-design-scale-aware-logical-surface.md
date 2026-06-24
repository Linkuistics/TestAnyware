# 16. HiDPI build design: a scale-aware logical RFB surface, opt-in over the host-scale path; deterministic mechanism deferred

Date: 2026-06-25

## Status

Accepted (planning leaf `target-shape-and-vision-disposition-k3`, grove
`hidpi-vision`). This ADR supplies the **build design** that ADR-0015 explicitly
deferred ("Detailed design deferred to the follow-on planning leaf"). ADR-0015
(feasibility verdict) is the settled floor; this ADR does not re-litigate it.

## Context

ADR-0015 established that HiDPI/Retina on a macOS VF guest is a **host-side**
display-config concern (guest-side REFUTED), and that a *deterministic* 2×
framebuffer (headless, on 1× hosts, in CI) requires injecting an explicit high
`pixelsPerInch` into `VZMacGraphicsDisplayConfiguration` — which shipped tart
cannot do (it hardcodes `pixelsPerInch: 72` on the `px`/headless path and inherits
the host monitor's backing scale on the `pt` path). The only mechanisms are a
**tart fork** or a **custom VF harness**, whose ongoing maintenance cost ADR-0015
flagged for this leaf to weigh against the grove's lone driver, **test realism**
(plan-k1 D1). The downscale design applies: a 2× config of logical 1920×1080 makes
the host RFB framebuffer report **px** 3840×2160, to be fed to vision as its native
1920×1080 px by an exact 2:1 downsample, with pointer events mapped ×2.

Codebase exploration during the grilling established two facts that reshape the
build:

1. The **consumer-side scaling** (2:1 downsample + pointer ×2) is
   **mechanism-independent and reusable**, and concentrates at a single chokepoint:
   `testanyware-rfb`'s `RfbConnection` owns the only `Framebuffer`, and every
   consumer reads framebuffer pixels/size and writes pointer events through it
   (`connection.rs:188`, `framebuffer.rs:13`, `connection.rs:257`). The tart
   fork / custom harness is the *expensive, low-reuse* part.
2. Shipped tart **already** reaches a 2× framebuffer **non-deterministically** via
   its `pt` display path, which inherits the host monitor's backing scale
   (`Sources/tart/Platform/Darwin.swift`'s
   `VZMacGraphicsDisplayConfiguration(for: hostMainScreen, sizeInPoints:)`). On a
   Retina dev Mac, `vm start --display 1920x1080pt` yields a 3840×2160 framebuffer
   today, with no VM-tool change.

## Decision

**Build the reusable scale-aware RFB surface now and ship HiDPI as a documented
opt-in over the existing host-scale (`pt`) mechanism; defer the deterministic
tart-fork / custom-VF-harness to a future, demand-triggered leaf.**

Concretely:

- **(D1) Minimal opt-in, defer the fork.** The deterministic host-side ppi
  injection (the only path to headless / 1×-host / CI HiDPI) is **deferred**. It is
  not a live leaf in this grove — it is recorded here as future work, triggered by
  real demand for headless/CI HiDPI. ADR-0015's mechanism survey (fork tart vs
  custom VF harness) is the starting point for that leaf when it is taken up. The
  1× default (ADR-0013/0014) is untouched; HiDPI is an opt-in alternative.

- **(D2) Scale lives in a scale-aware `RfbConnection` presenting a logical
  surface.** The connection negotiates the physical 3840×2160 on the wire but
  presents a **logical 1920×1080** surface to all consumers: downsample
  physical→logical on framebuffer reads (exact 2:1 box-average), multiply
  logical→physical on pointer writes. The scale factor is `physical_w / logical_w`,
  **auto-detected per connection** — `1` is a no-op, so the *same code path*
  degrades gracefully on a 1× host where HiDPI did not actually happen. Every
  consumer (vision via `screen find-text`, `screen size/capture/record`, the
  embedded viewer, `input click`) operates in one uniform coordinate space and is
  otherwise unchanged. The element-based a11y path (agent endpoints) never touches
  framebuffer coordinates and is unaffected.

- **(D2b) `screen capture` / `screen record` default to logical, with a
  `--physical` opt-in** that emits the raw 3840×2160 Retina frame. The logical
  default keeps capture/record coordinate-consistent with `--region`, clicks, and
  vision; `--physical` preserves access to the pixel-exact realism artifact.

- **(D3) Opt-in surface: `--display WxH@2x`**, a scale suffix we parse and
  translate ourselves (tart never sees `@2x`). It is **mechanism-agnostic**: today
  it maps to the host-scale `pt` path; when the deferred mechanism lands it routes
  there, with the user surface unchanged. Constrained to **integer `@2x`** (only an
  exact 2:1 downsample lands on the vision distribution; fractional scaling is out
  of scope). The flag (1) routes tart to the host-scale path, (2) **suppresses
  ADR-0014's guest-side 1× switch** (`display::apply`, `lifecycle.rs:229`), and
  (3) sets the connection's logical target.

- **(D4) Vision runs on the logical surface; the on-distribution *claim* is gated
  on a verify leaf.** The connection feeds vision a correctly-dimensioned 1920×1080
  frame, so vision runs on the HiDPI path. But a downsampled-2× frame carries @2x
  assets, retina font hinting, and heavier antialiasing that the native-1× synthetic
  training set never saw (`scenario_library.py:5`), so on-distribution-ness is
  **empirical, not asserted**. The verify leaf `verify-vision-on-downsampled-2x`
  measures OCR + window-detection accuracy against the native-1× baseline. Pass →
  vision-on-HiDPI is blessed; material fail → HiDPI remains a realism/viewer path
  and vision-parity becomes a retraining workstream (ADR-0013's "separate
  workstream").

## Considered options

- **Full deterministic feature now (fork tart / custom VF harness up front).**
  *Rejected for now.* It pays the fork's ongoing maintenance tax (rebase-forever
  for a tart fork; a new `VmTool` backend reopening ADR-0010's deliberately
  two-armed `{Tart, Qemu}` dispatch for a custom harness) before any demand for
  headless/CI HiDPI exists. The realism driver is satisfied for a developer on a
  Retina Mac by the far cheaper `pt` path.
- **Design-only, defer all build.** *Rejected* — it forgoes the reusable value (the
  scale-aware surface is mechanism-independent and useful immediately on a Retina
  dev host) for no saving, since the design is already settled here.
- **Downsample only in the vision path / per-consumer.** *Rejected* (see D2). The
  vision-only option splits the coordinate space (vision in 1920, everything else
  in 3840) and moves the ×2 to the vision→click handoff — a footgun. Per-consumer
  duplicates the logic across five consumers with inconsistency risk.
- **Reuse `--display WxHpt` as the opt-in trigger.** *Rejected* (see D3) — it
  overloads a cryptic unit suffix as a feature toggle (the ADR-0013 footgun) and
  leaks today's mechanism, becoming wrong after the deferred fork lands.

## Consequences

- HiDPI works **today on a Retina dev Mac** via `vm start --display 1920x1080@2x`
  with no VM-tool change. It does **not** work headless or on a 1× host until the
  deferred deterministic mechanism is built; the opt-in must **detect** the actual
  scale (physical/logical) and **warn** when HiDPI was requested but the host
  yielded 1× — the auto-detect makes this safe, never silently wrong.
- The reusable artifact is a **scale-aware RFB surface** in `testanyware-rfb`,
  built and unit-testable on any host (feed a synthetic 3840×2160 frame, assert a
  1920×1080 logical read and ×2 pointer write). It carries a per-frame downsample
  cost — 3840×2160 RGBA is ~33 MB/frame — that the viewer/`screen record` loops
  must absorb (ADR-0013 already flagged frame size); a build-leaf concern.
- One **empirical unknown** gates end-to-end behaviour and the opt-in's exact
  `vm start` sequencing: under the `pt` path on a Retina host, does the guest
  WindowServer *select* the Retina 1920-logical mode, or restore its golden-baked
  1024×768 and need a guest-side switch to the Retina mode? ADR-0015 derived (did
  not measure) the 3840×2160 framebuffer for want of a Retina host. The build leaf
  `confirm-hidpi-pt-path-on-retina-host` resolves this first — it is the empirical
  doubt pass for the load-bearing claim.
- ADR-0013's 1× `1920x1080px` default and ADR-0014's guest-side 1× runtime switch
  **stand unchanged** as the default; the HiDPI opt-in *suppresses* the ADR-0014
  switch only when `@2x` is requested.

## Deferred (future work, not a live leaf in this grove)

**Deterministic host-side HiDPI** — a tart fork (add a ppi / `@2x` display option)
or a custom VF harness (the `s-u/macosvm` `dpi` model). Required only for headless
/ 1×-host / CI HiDPI. The `--display WxH@2x` surface (D3) is built mechanism-
agnostic precisely so this can be slotted underneath later without changing the
user surface or the scale-aware connection. Start from ADR-0015's "Considered
options" survey. Trigger: concrete demand for reproducible HiDPI off a Retina host.

## Verification (2026-06-25, `confirm-hidpi-pt-path-on-retina-host-k4`)

**Verdict: CONFIRMED.** The `pt`→2× path delivers a 3840×2160 framebuffer, and
the `vm start` sequencing the k6 opt-in needs is pinned. This closes the one
empirical unknown above (and ADR-0015's derived-not-measured Q2).

The dev host is a 1× 5120×2160 ultrawide (`NSScreen.main.backingScaleFactor ==
1.0`) — the same 1× host that forced ADR-0015 to *derive* rather than *measure*.
But the host **panel** offers a native `2560×1080 pt | 5120×2160 px | 2.00×`
HiDPI mode, so the host was temporarily switched into it (a fresh-process
`NSScreen.main.backingScaleFactor` then reads `2.0` — exactly what tart's `pt`
path consults at VM-construction) for the measurement and restored to 1×
afterward. Method otherwise mirrors ADR-0014/0015: a fresh
`testanyware-golden-macos-tahoe` clone started with `--display 1920x1080pt`;
host-compiled CoreGraphics probes `/upload`+`/exec`'d over the agent; framebuffer
read via `testanyware screen size`. No golden regeneration.

1. **`pt`→2× advertises Retina modes (the two-axis invariant, now both axes
   measured).** With the host at 2×, the guest advertised **34 modes, 18 of them
   2× (Retina)** — including `1920×1080 pt | 3840×2160 px | scale 2.00`. Against
   k2's run of the *same* `pt` config on a 1× host (12 modes, **all scale 1.0**),
   this isolates ADR-0015's invariant cleanly: the config's **point dimensions**
   set the advertised *sizes*; the **host backing scale** sets their *scale*.

2. **The 3840×2160 framebuffer is measured, not derived.** Switching the guest's
   main display to the Retina 1920-logical mode made `screen size` report
   **3840×2160 px**, with the active CG mode `1920×1080 pt | 3840×2160 px | scale
   2.00`. A 2× config of logical 1920×1080 ⇒ a 3840×2160-px host RFB framebuffer.
   **The downscale design's px precondition (plan-k1 D2) holds, empirically.**

3. **Sequencing: the guest does NOT auto-select Retina — it restores the golden's
   saved 1024×768.** Right after `vm start`, `screen size` = 1024×768 and the
   active CG mode was `1024×768 | scale 1.00`. So k6 **must issue a guest-side
   switch** to *select* the Retina mode (WindowServer's saved-mode restore is
   scale-independent — the same behaviour ADR-0014 found at 1×). The exact
   selector k6 implements: the first advertised mode with **`pixelWidth ==
   2·logicalW && pixelHeight == 2·logicalH && width == logicalW`** (scale 2.0),
   applied via the persistent `.forSession` transaction
   (`CGBeginDisplayConfiguration → CGConfigureDisplayWithDisplayMode →
   CGCompleteDisplayConfiguration`) — the same pattern `set-display-mode.swift`
   uses. Confirmed working: VF resized the host framebuffer to 3840×2160 within
   ~1 s of the switch. **Select by predicate, never by `modeID`** — mode ids are
   not stable across boots or golden regens.

4. **ADR-0014's 1× switch MUST be suppressed under `@2x` — and the reason is
   sharper than the leaf brief assumed.** The brief hypothesised the 1× selector
   (`pixelWidth==w && width==w`) would have *no match* at 2×. **Refuted:** at 2×
   the guest advertises **both** a 1× `1920×1080` mode (`px==pt==1920`) *and* the
   2× `1920×1080`-logical mode (`px==3840`). So ADR-0014's `set-display-mode 1920
   1080` would **match the 1× mode and succeed**, forcing the guest to LoDPI
   1920×1080 and **silently defeating HiDPI**. Suppression (D3 item 2) is
   therefore load-bearing, not a tidy-up: the switch is **not** self-disabling via
   no-match; left enabled it actively selects the wrong mode. (In this run the
   switch happened to be a no-op — the agent IP was not ready inside `vm start`'s
   window, so `display::apply` was skipped, which is why the pristine 1024×768 was
   observable. The selector's match against the real advertised 1× mode is what
   matters for k6, independent of that timing.)

**Net for the build:** k6's `@2x` path = route to the `pt` path (passes through
today) → **suppress** ADR-0014's 1× switch → **issue** a guest-side Retina-mode
switch (predicate above, `.forSession`) → set the scale-aware connection's
logical target. k5's scale-aware surface is unaffected (mechanism-independent,
unit-testable on any host).
