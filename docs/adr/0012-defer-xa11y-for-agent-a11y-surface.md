# Defer xa11y for the in-VM agents' a11y surface

**Status:** Rejected (deferred) — revisit at xa11y ≥ 1.0

## Decision

We will **not** adopt [xa11y](https://github.com/xa11y/xa11y) to replace or
augment the *Agent a11y surface* (the accessibility-tree subset of the in-VM
agents) at this time. The three per-platform agents keep their current native
implementations (Linux=Python/AT-SPI2, macOS=Swift/AXUIElement,
Windows=C#/UI Automation). The blocking factor is **API instability, not
capability** — so this is a *deferral*, not a permanent rejection: reconsider
when xa11y ships **≥ 1.0 with a stabilized public API** and **third-party usage
evidence** appears.

Per-platform verdicts (the grove's per-platform Done-when):

| Platform | Verdict | Driver |
|---|---|---|
| macOS | augment-capable, **deferred** | Only platform with primary real-world evidence; full Tier-1 API coverage. Blocked by the global API-stability gate, not by a capability gap. |
| Windows | augment-capable but **unproven**, deferred | Identical shared API + best-structured native data model, but zero demos/dependents. |
| Linux | **reject** | Maintainer documents AT-SPI2 tree reads as "far too slow for a single step" — the `snapshot` anchor is too slow as a drop-in. |

**Roll-up (per the grove's majority rule):** "replace" is only worth committing
on a majority of platforms. xa11y's pre-1.0 churn caps *every* platform at
"augment" today, so a majority-replace is unreachable — and a one/two-platform
augment would keep three agent languages *and* add a fast-moving dependency (the
"worst of both worlds"). Net: do not adopt now.

## Context & rationale

This records the outcome of the `investigate-xa11y` spike. Full evidence,
citations, and the gate/rubric scoring are in
[`docs/research/xa11y-maturity-and-coverage.md`](../research/xa11y-maturity-and-coverage.md).
The planned macOS snapshot-parity prototype (`030`) was **retired unstarted**:
once the decision is "don't adopt," there is nothing for a parity probe to prove.

The decision rests on four maturity/licensing gates applied before any fidelity
scoring:

- **Cadence/bus-factor — pass, with caveat.** Hyperactive (pushed daily, 13
  releases in ~10 weeks) but a **single human maintainer** on a **<3-month-old**
  repo (28 stars). Abandonment would leave us owning three native a11y backends
  in Rust — worse than today.
- **License — pass.** MIT throughout, with a CI-enforced `cargo-deny`
  permissive-only allowlist (copyleft excluded); backends (`windows`,
  `core-foundation`, `zbus`) are all permissive.
- **API stability — fails the "replace" bar (the binding constraint).** Pre-1.0
  with **six breaking 0.x minor releases in ~10 weeks** (0.3→0.8). This is why
  the verdict is *defer*: the gap is time/stability, not features.
- **Real-world evidence — asymmetric.** crates.io shows **0 reverse-deps**; the
  README's "several real world projects" has no locatable source. Concrete
  first-party evidence is a macOS Calculator demo only; Linux is documented as
  too slow.

Capability is genuinely close: xa11y covers all five Tier-1 endpoints, its
CSS-like selector engine is a *superset* of the agents' flat resolver, and its
element model reaches near-attribute parity. The one material fidelity gap is the
**role taxonomy** — xa11y normalizes to ~42 roles vs. the agents' ~115-value
`UnifiedRole` — but it is recoverable via xa11y's raw-platform-role escape hatch
feeding the agents' existing `RoleMapper`. The remaining hard gap is
**window-{resize,move,close,minimize}**, which xa11y does not expose at all
(a thin native shim would always be required). None of this is the reason to
wait; the pre-1.0 churn is.

## Consequences

- The three-language agent split persists for now. No new dependency is added.
- The RFB + OCR vision path is untouched (xa11y was never a competitor there).
- **Revisit trigger:** xa11y ≥ 1.0 with a stable public API and third-party
  adoption. At that point the natural re-entry is the deferred macOS
  snapshot-parity prototype, then an augment (not replace) integration that keeps
  the agents' `RoleMapper` and adds a native window-management shim.
