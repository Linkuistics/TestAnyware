# 010-plan

**Kind:** planning

## Goal

Walk the design tree for the xa11y investigation and **grow the grove**: decide
how to structure the spike — what to research, what to prototype, and what the
per-platform decision gates are — without pre-judging the replace/augment/reject
outcome.

## Context

Seeded from the inbox observation (now incorporated into the root `BRIEF.md`).
Founding facts already established by code inspection:

- Three per-platform agents in three languages (Linux=Python, macOS=Swift,
  Windows=C#) each reimplement the same a11y HTTP surface.
- The a11y surface xa11y could plausibly cover:
  `windows · snapshot · inspect · press · set-value · focus · show-menu ·
  window-{focus,resize,move,close,minimize} · wait`.
- Non-a11y endpoints (`/exec`, `/upload`, `/download`, `/shutdown`, `/health`)
  and the agent process/HTTP envelope stay regardless.

## Open questions (the design tree to grill)

1. **Spike shape** — research-leaf first (prior-art / maturity survey of xa11y)
   before any prototyping, or jump to a hands-on prototype on one platform?
2. **Decision unit** — one cross-platform decision, or genuinely per-platform
   replace/augment/reject (the agents are per-platform today)?
3. **Prototype target** — if we prototype, which platform first and what's the
   minimal proof (e.g. selector parity for `inspect`/`snapshot` on one app)?
4. **Fidelity bar** — what must xa11y's selector/action model match for "replace"
   to be on the table vs. "augment"? Where are the known gaps (show-menu,
   window-* geometry, wait semantics)?
5. **Maturity/licensing gate** — what evidence (version, release cadence, issue
   history, dep license audit) would be disqualifying before we invest?
6. **Consumer question** — are the agents even the right consumer, or is a
   vision-pipeline / computer-use grove the better home for xa11y?

## Done when

The grove tree is grown with the right next leaves (research and/or prototype),
decision gates are explicit, and `CONTEXT.md` carries any new terms (e.g.
*xa11y*, *a11y surface*). Decisions recorded inline below as they settle.

## Decisions (running log)

**Q1 — Spike shape: research-leaf-first (settled).** A prior-art / maturity /
API-coverage research leaf runs *before* any prototype and gates it. Rationale:
xa11y's maturity is the single biggest disqualifier and a doc/issue survey is
cheap; a prototype built first risks being wasted if licensing/maturity is
disqualifying. The research leaf's brief will name the downstream questions
(selector-model coverage of `inspect`/`snapshot`, `show-menu` equivalents,
window geometry, `wait` semantics) and bias toward post-mortem-style evidence
(open issues, release cadence) over a feature list. A narrow one-platform
prototype leaf may follow, gated on the research outcome.

**Q2 — Decision unit: per-platform verdicts + roll-up rule (settled).** Each
platform gets its own replace/augment/reject verdict (the agents are already
per-platform; the three native frameworks — AXUIElement / UI Automation /
AT-SPI2 — vary too much for one global verdict). Shared research produces all
three verdicts; only optional prototypes split per platform. **Roll-up rule:**
"replace" is only worth committing if it lands on a majority of platforms — a
single-platform replacement keeps three languages *and* adds a dependency
(worst of both worlds), so it rolls up to reject-everywhere unless it clears
that bar.

**Q3 — Fidelity bar: three-tier rubric, scored per endpoint per platform
(settled).**
- *Tier 1 (must cover or it's an immediate reject, no augment):* `snapshot`,
  `inspect`, `press`, `focus`, `set-value` — xa11y's advertised selector +
  action-synthesis model.
- *Tier 2 (gaps downgrade replace→augment, not reject):* `show-menu`,
  `window-{resize,move,close,minimize}` — context-menu + window-geometry
  manipulation, arguably window-management rather than a11y-tree; xa11y may
  expose these read-only. Gap ⇒ keep a thin native shim, xa11y does the rest.
- *Tier 3 (out of scope, never scored):* `wait` (our polling/retry wrapper),
  `/exec` `/upload` `/download` `/shutdown` `/health`.
- **Load-bearing caveat:** `snapshot` fidelity is *attribute-level*, not binary.
  The host CLI consumes a specific JSON shape (roles, names, geometry,
  hierarchy) and its selectors break if xa11y can't expose the same attributes.
  Research must score attribute coverage, not just "has a snapshot call" — the
  most likely place a confident "replace" degrades to "augment + translation
  layer."

**Q4 — Maturity/licensing gate: four hard gates, applied first (settled).**
Any failure ⇒ reject regardless of fidelity; the research leaf short-circuits
on these before scoring the rubric.
1. *Bus-factor/cadence (global):* single-maintainer + no commits in ~6 months ⇒
   reject. Abandonment leaves us owning three native a11y backends in Rust —
   worse than today.
2. *License purity, transitive (global):* audit the full per-platform dep tree
   (`cargo tree` + license scan), not just xa11y's own MIT. One viral/copyleft
   backend dep can disqualify.
3. *API stability (global):* pre-1.0 + churning public API ⇒ reject for
   "replace" (acceptable for a throwaway prototype only).
4. *Real-world evidence (per-platform):* primary evidence the unified API drives
   real apps on *that* platform (issues, demos, dependents) — README claims
   don't count. Absence ⇒ that platform defaults toward reject. This asymmetry
   (xa11y may be proven on macOS/Windows but aspirational on AT-SPI2/Linux)
   feeds the Q2 per-platform verdicts directly.

**Q5 — Prototype target: macOS-first `snapshot` attribute-parity probe
(settled, gated).** First prototype targets macOS (AXUIElement is the most
coherent native framework — where "replace" is most likely true and a flaw
shows early; golden macOS VM is cheap clone+start; live Swift agent to diff
against). Minimal proof = run xa11y vs. the live `/snapshot` on one real app and
**diff the trees** for attribute parity (roles/names/geometry/hierarchy the host
CLI's JSON shape needs) — highest-information, lowest-cost probe per Q3. *Not* a
full agent rewrite. The prototype leaf is created now but its brief gates it:
"do not start until 020-research clears all four Q4 gates"; retires unstarted if
research rejects.

**Q6 — Consumer question: agents are the right and only in-scope consumer
(settled, closed).** xa11y is a11y-tree automation; the vision-pipeline /
ocr-accuracy groves work the opposite modality (pixels/framebuffer/OCR) and are
*complementary, not competing* homes. A computer-use grove that fuses both is a
downstream consumer of the agent's a11y output, not a rival home for xa11y. The
agents are the only place TestAnyware touches native a11y trees, so the question
resolves cleanly. Value of asking: it reconfirms the RFB+OCR vision path stays
untouched regardless of this grove's outcome. *Second-order tiebreaker (note,
not driver):* a Rust xa11y agent shares a language with the Rust host CLI
(`cli-rs/`), a mild plus for a future unified-codebase argument even if
per-agent consolidation alone doesn't justify "replace."

---

### Tree grown from this session

- `020-research-xa11y-maturity-and-coverage` — research leaf: apply the four Q4
  gates (short-circuit), then score the Q3 three-tier rubric per platform;
  produce three replace/augment/reject verdicts + a roll-up recommendation,
  citation-backed, into `docs/research/`.
- `030-prototype-macos-snapshot-parity` — work leaf, **gated on 020** clearing
  all four gates: diff xa11y's snapshot vs. the live Swift agent's `/snapshot`
  on one real app for attribute parity. Retires unstarted if research rejects.
- `040-decide-and-record` — synthesis leaf: fold 020 + 030 into the per-platform
  verdict + roll-up, recorded as an ADR (the grove's deliverable / Done-when).
