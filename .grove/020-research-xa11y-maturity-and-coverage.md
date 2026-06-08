# 020-research-xa11y-maturity-and-coverage

**Kind:** work (research leaf)

## Goal

Produce a citation-backed research note that answers, per platform, whether
xa11y can **replace / augment / reject** the *Agent a11y surface*. This note
gates the rest of the grove: 030 (prototype) only runs if xa11y clears the
gates below, and 040 (decide) synthesizes from it.

Output: `docs/research/xa11y-maturity-and-coverage.md`.

## Method — gates first, then rubric (do not skip the order)

**Step 1 — Apply the four maturity/licensing gates. Any failure ⇒ reject;
short-circuit and stop scoring that platform (or the whole investigation for
global gates).**

1. *Bus-factor / cadence (global):* contributor count, commit recency, release
   tags. Single-maintainer + ~6 months stale ⇒ reject.
2. *License purity, transitive (global):* audit the full per-platform dep tree
   (`cargo tree` + a license scan), not just xa11y's own MIT. One
   viral/copyleft backend dep can disqualify.
3. *API stability (global):* pre-1.0 + churning public API ⇒ reject for
   "replace" (note it as prototype-only-acceptable).
4. *Real-world evidence (per-platform):* primary evidence (issues, demos,
   dependent projects) the unified API drives real apps on *that* platform.
   README claims don't count — cite primary sources or record "no source found"
   (absence ⇒ that platform defaults toward reject).

**Step 2 — Score the three-tier fidelity rubric, per endpoint per platform.**

- *Tier 1 (must cover or immediate reject, no augment):* `snapshot`, `inspect`,
  `press`, `focus`, `set-value`.
- *Tier 2 (gap ⇒ replace→augment, keep a thin native shim):* `show-menu`,
  `window-{resize,move,close,minimize}`.
- *Tier 3 (out of scope, don't score):* `wait`, `/exec`, `/upload`,
  `/download`, `/shutdown`, `/health`.
- **`snapshot` is attribute-level, not binary:** check whether xa11y exposes
  the roles / names / geometry / hierarchy attributes the host CLI's snapshot
  JSON depends on. "Has a snapshot call" is insufficient — missing attributes
  push replace→augment (with a translation layer).

## Context

Decisions Q1–Q6 were settled in `010-plan.md` (running log); this leaf
executes the research arm. The agents' real surface was confirmed by code
inspection (see root `BRIEF.md` + `CONTEXT.md` → *Agent a11y surface*). Ground
the rubric in the live endpoints:
`agents/windows/AccessibilityEndpoints.cs`,
`agents/macos/Sources/testanyware-agent/AgentServer.swift`,
`agents/linux/testanyware_agent/`.

Watch for the per-platform asymmetry: xa11y may be proven on macOS/Windows but
aspirational on AT-SPI2/Linux.

## Done when

`docs/research/xa11y-maturity-and-coverage.md` exists with: a gate result per
global gate + per platform; a rubric score per Tier-1/Tier-2 endpoint per
platform; a snapshot attribute-coverage finding; three per-platform
replace/augment/reject verdicts; a roll-up recommendation (per Q2's rule:
replace only worth it on a majority of platforms); every failure-mode and
coverage claim carries a primary-source citation or an explicit "no source
found" note.

## Notes

If a global gate fails, the grove may go straight to 040 and finish as
"reject" — flag that in the note so the operator can short-circuit 030.
