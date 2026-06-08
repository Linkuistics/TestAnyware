# 040-decide-and-record

**Kind:** planning (synthesis / decision)

## Goal

Fold `020-research` (per-platform verdicts) and `030-prototype` (macOS parity
result, if it ran) into the grove's **deliverable**: a per-platform
replace/augment/reject decision plus the Q2 roll-up, recorded as an ADR. This
is the leaf that satisfies the root `BRIEF.md` "Done when."

## Method

1. Read 020's research note and 030's findings note (or 030's
   retired-unstarted status).
2. Apply the **roll-up rule (Q2):** "replace" is only worth committing if it
   lands on a majority of platforms; a single-platform replace keeps three
   languages *and* adds a dependency, so it rolls up to reject-everywhere unless
   it clears that bar.
3. Write an ADR under `docs/adr/` recording the decision per platform + the
   roll-up, with the research citations as the rationale (per `driving.md`:
   research that *changed* a decision is cited in the ADR's rationale).
4. If the decision is anything other than reject-everywhere, **grow follow-up
   work leaves** here for the actual port/augment work (per platform). If it is
   reject-everywhere, no follow-up leaves — the grove is ready to finish.
5. Promote "xa11y" into `CONTEXT.md` *only if* the decision keeps it in the
   codebase (held back during planning as lazy/optional since it may be
   rejected).

## Context

This is where the grilling's *method* (gates + rubric + roll-up, settled in
`010-plan.md`) becomes a *decision*. The methodology itself was deliberately
not ADR'd during planning — the durable ADR is the verdict, written here with
evidence in hand.

## Done when

An ADR records the per-platform verdict + roll-up with cited rationale;
follow-up work leaves exist if the verdict warrants them; the grove is ready to
finish (or has clear next leaves).

## Notes

Consider whether the verdict deserves a PRD (human-facing agreement point) in
addition to the ADR — only if it commits real downstream work across platforms.
