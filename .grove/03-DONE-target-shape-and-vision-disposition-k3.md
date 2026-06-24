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

## Decisions (running log)

**D1 — Ambition: minimal opt-in now, defer the fork.** Deterministic HiDPI
(headless / 1× hosts / CI) requires a host-side ppi injection — a tart fork or a
custom VF harness — whose maintenance cost ADR-0015 flagged for this leaf to
weigh. Codebase exploration established that the *consumer-side* scaling work
(2:1 downsample + pointer ×2) is **mechanism-independent and reusable**, and that
the shipped tart already reaches 2× **non-deterministically** via its `pt`
display path (inherits the host monitor's backing scale — `Darwin.swift`'s
`VZMacGraphicsDisplayConfiguration(for: hostMainScreen, …)`). So: build the
reusable scaling machinery now and gate HiDPI behind the existing `pt` path as a
**documented dev-convenience opt-in** (Retina host required; scale auto-detected,
no silent wrong behaviour); **defer** the deterministic tart-fork/custom-harness
to a future demand-triggered leaf. The brief's Q1 (fork vs harness) is therefore
*not decided here* — it moves to that deferred leaf, carrying ADR-0015's mechanism
survey. The 1× default (ADR-0013/0014) is untouched.

**D2 — Scale lives in a scale-aware RFB connection presenting a logical
surface.** `testanyware-rfb`'s `RfbConnection` owns the only `Framebuffer` and is
the sole path for both framebuffer reads (`connection.rs:188`, `framebuffer.rs:13`)
and pointer writes (`connection.rs:257`). So the connection negotiates the
physical 3840×2160 on the wire but presents a **logical 1920×1080** surface:
downsample physical→logical on reads (exact 2:1 box-average), multiply
logical→physical on pointer writes. Scale = `physical_w / logical_w`, auto-detected
per connection — `1` is a no-op, so the same path degrades gracefully on a 1×
host. Every consumer (vision via `screen find-text`, `screen capture/size/record`,
the embedded viewer, `input click`) stays in one uniform coordinate space and is
otherwise unchanged; the element-based a11y path (agent endpoints) never touches
framebuffer coordinates and is untouched. Rejected: vision-path-only downsample
(splits the coordinate space, moves ×2 to the vision→click handoff) and
per-consumer scaling (five duplicated impls). Build note: 3840×2160 RGBA is
~33 MB/frame, so the per-frame downsample in the viewer/record loops has real CPU
cost (ADR-0013 already flagged frame size) — a build-leaf concern, not a blocker.

**D2b — capture/record default logical, `--physical` opt-in.** `screen capture` /
`screen record` default to the logical 1920×1080 (downsampled) frame — uniform
with `screen size`, `--region`, clicks, and vision — and gain a `--physical` flag
that emits the raw 3840×2160 Retina frame for the pixel-exact realism artifact. A
2:1 box-downsample of a 2× render still carries the smoothing/@2x assets, so the
logical capture is already "more realistic" than today's 1× render; `--physical`
is for "what a Retina user literally sees."

**D3 — opt-in surface: `--display WxH@2x` scale suffix.** A scale suffix on the
existing `WxH[pt|px]` display grammar, reading as "logical WxH at 2× backing
scale." We parse and translate it ourselves — **tart never sees `@2x`**: today it
maps to the host-scale `pt` path; when the deferred deterministic mechanism lands
it routes there instead — a **stable, mechanism-agnostic** user surface that
survives the swap (the cryptic `pt` unit would leak today's mechanism and be wrong
after the fork). Constrained to **integer `@2x`** — only an exact 2:1 downsample
lands cleanly on the vision distribution; fractional Mac "scaled" modes (`@1.5x`)
are out of scope. The flag does three things: (1) route tart to the host-scale
path, (2) **suppress ADR-0014's guest-side 1× switch** (`display::apply`,
`lifecycle.rs:229` — under a 2× config its `pixelWidth==w && width==w` selector
finds no mode; whether it must instead *select* the Retina 1920-logical mode is the
empirical question the first build leaf resolves), (3) set the scale-aware
connection's logical target. Default stays 1× `1920x1080px` (ADR-0013).

**D4 — vision: run it on the logical surface, gate the parity claim on a verify
leaf.** The scale-aware connection feeds vision a correctly-*dimensioned* 1920×1080
frame, so vision *runs* on the HiDPI path. But "right dimensions ≠ on-distribution":
a downsampled-2× frame carries @2x assets / retina hinting / heavier AA that the
native-1× synthetic training set (`scenario_library.py:5`) never saw. This is
empirical, so we provisionally assert on-distribution and **gate the claim** behind
a named verify leaf `verify-vision-on-downsampled-2x` measuring OCR (`find-text`) +
window-detection accuracy vs the native-1× baseline. Pass → bless vision-on-HiDPI;
material fail → HiDPI stays a realism/viewer path and vision-parity becomes a
retraining workstream (ADR-0013's "separate workstream"). Because vision degrading
doesn't break the realism/viewer value (D1), we don't pre-cripple it.

## Artifacts + decomposition (this session)

- **ADR-0016** records the build design (D1–D4) and the deferred deterministic
  mechanism. **No PRD** — this is an internal display-pipeline decision with no
  broader product-agreement audience; the ADR + this running log are the record
  (driving.md: avoid the session-end decision-summary anti-pattern).
- **CONTEXT.md** gains `[[HiDPI logical framebuffer]]`; the
  `[[Framebuffer-pixel contract]]` "deferred" framing is updated to point at this
  opt-in.
- The root `BRIEF.md` "Done when" is tightened into a concrete bar and lists the
  grown leaves; the deferred mechanism is a BRIEF/ADR pointer, **not a live leaf**
  (a live leaf would force the self-driving loop to build the fork, contradicting
  D1 — the grove must stay finishable on the minimal-opt-in scope).
- New leaves grown at the root: `confirm-hidpi-pt-path-on-retina-host` (k4, the
  load-bearing empirical de-risk — it *is* the doubt pass for the derived-not-
  measured `pt`→2× claim), `build-scale-aware-rfb-connection` (k5),
  `build-hidpi-optin-and-wiring` (k6), `verify-vision-on-downsampled-2x` (k7).
