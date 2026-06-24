# plan-k1

**Kind:** planning

## Goal

Scope and **decompose** the macos-guest-resolution grove into leaves, and settle
the design sub-decisions ADR-0014 left open. The *what* (runtime upload+exec
CoreGraphics helper) is decided; this leaf decides the *how and in what order*.

## Context

- ADR-0014 decided the mechanism and **deferred implementation to this grove**.
- It names a **load-bearing empirical unknown** (does VF advertise a selectable
  1920×1080 CG display mode for a headless guest?) and is emphatic: **de-risk
  with a spike before building**.
- Code touchpoints mapped 2026-06-24 (see root BRIEF.md Pointers).

## Open decisions to grill

1. **Grove shape** — spike → build → verify decomposition (leaves vs node).
2. **Spike form** — throwaway vs committed probe artifact; run it in-grove now?
3. **Contingency** if the spike says VF won't advertise 1920×1080.
4. **Helper delivery** — per-start upload vs cached-in-guest vs native endpoint.
5. **Readiness gating & the transient** — does `vm start` block on the switch?
   failure mode (degraded agent ⇒ leave at 1024×768 + warn?).
6. **`--display` footgun** — fix the user-facing macOS pt/px footgun here, or
   leave out of scope?

## Done when

- The tree is grown (leaves created) and the open decisions above are settled
  in the running log below (with ADRs raised only where they earn it).

## Decisions (running log)

<!-- append one paragraph per settled question, at the moment it settles -->

**D1 — Spike-first, empirical, hard gate.** Leaf 1 is a **work-task spike** that
clones+starts the live macOS golden (host is darwin; golden kept built), uploads
a probe through the agent `/upload`+`/exec` surface that dumps
`CGDisplayCopyAllDisplayModes` and attempts `CGDisplaySetDisplayMode` to
1920×1080, and captures the **real** result. Build leaves are grown/sequenced
only after the spike resolves the binary unknown (ADR-0014 unknown #1). Settles
ADR-0014's emphatic "de-risk with a spike before building" with data, not a
guess.

**D2 — Spike output: ADR-0014 verification section + kept probe.** The spike
appends a dated **Verification** section to ADR-0014 (modes VF advertised;
whether 1920×1080 was present & selectable; `CGDisplaySetDisplayMode` result;
CONFIRMED/REFUTED verdict), mirroring ADR-0013's verification-section precedent
(its work leaf recorded the 1024×768 finding in the ADR itself). The probe
`.swift` is **kept committed** under `provisioner/helpers/` — its
`CGDisplayCopyAllDisplayModes`/`CGDisplaySetDisplayMode` calls seed the real
helper, so the build leaf reuses already-verified CG code.

**D3 — Gate encoded in briefs; pivot cheaply on refute.** Grow the happy-path
build leaves now; their briefs state "presupposes spike CONFIRMED unknown #1".
The spike leaf's brief instructs: on REFUTE, `leaf-insert` a fallback-replan
leaf ahead of the build leaves and mark them void (the known fallbacks — private
CGS mode-creation API, or golden-bake — are named so the pivot isn't from
scratch). No speculative fallback leaf is grown now (lazy, constraint 4);
1920×1080 is near-universal so CONFIRM is the likely outcome.

**Deferred (build-leaf-internal, not pre-decided — lazy):**
- *Helper delivery*: default **per-start upload** (ADR-0014); native `/set-display`
  endpoint reconsidered only if per-start proves a problem.
- *Readiness + failure mode*: wait for agent readiness, then ride the existing
  **optionally-degraded** contract — a not-ready agent ⇒ warn + leave 1024×768,
  do **not** fail `vm start` (consistent with today's `agent_unreachable` path).
- *Transient*: `vm start` performs the switch **synchronously before returning
  success**, so any consumer waiting on `vm start` is gated by construction — no
  separate vision/screen gating work needed.

**D4 — Target is 1920×1080 px @1× (LoDPI); HiDPI deferred to its own grove.**
Grilling pushback surfaced "don't we want HiDPI / 1920×1080 *points*?" The
resolution: the binding contract is **framebuffer pixels** (ADR-0013), and
**layout parity holds at *both* 1× and 2×** (a window's fraction-of-screen is a
points property, identical either way) — so the px-vs-pt choice is decided by
the *vision pipeline*, not layout. We target a **1920×1080 mode at backing scale
1.0**: `pixelWidth=1920` (on-distribution) **and** `width=1920 pt` (same window
sizes as Linux). The build helper is **parameterized by target px** so a future
HiDPI grove is a one-line flip. RFB transports pixels generically (no protocol
or viewer-protocol change); HiDPI's cost lands on consumers (4× bytes/frame,
vision retrain/downscale, viewer host-Retina handling) — captured as a **separate
follow-on grove** (`hidpi-vision`, suggested), not folded in here. The **spike
must report pt + px + scale per advertised mode** so we learn whether a true 1×
1920×1080 mode exists; if VF offers only Retina, the strategic question is forced
and we revisit. Glossary updated inline: `[[Framebuffer-pixel contract]]`.

**D5 — `--display` pt/px footgun: out of scope.** The footgun question (ADR-0013)
was superseded mid-grill by the deeper target decision (D4); its disposition
falls out of D4's "this grove targets the **default** path" framing. The
pre-existing macOS explicit-`--display` footgun is **not** fixed here — the build
leaf documents the interaction (the resolved value now feeds both `tart set` and
the helper) but leaves ADR-0013's deliberate non-fix standing.

## Tree grown

- `02-spike-display-modes-k2` — the gate (work/spike).
- `03-build-resolution-switch-k3` — helper + `vm start` wiring + live verify
  (work); brief presupposes spike CONFIRMED.

## Notes
