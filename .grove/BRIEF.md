# investigate-xa11y — brief

## Goal

Decide whether [xa11y](https://xa11y.dev/) — a cross-platform, accessibility-tree
UI-automation library (one unified API over AXUIElement / UI Automation / AT-SPI2,
with a Rust binding) — should **replace, augment, or be rejected** for the
per-platform accessibility-tree code inside TestAnyware's *in-VM agents*.

This is an **investigation/spike**, not a committed port. The deliverable is a
decision (per platform), grounded in the agents' real surface and in xa11y's
maturity/licensing, recorded durably.

## Scope boundary (settled at seed time)

- **In scope:** the a11y-tree subset of the in-VM agents
  (`agents/{linux,macos,windows}/`) — today three separate implementations:
  Linux=Python, macOS=Swift, Windows=C#. The shared a11y surface is
  `windows · snapshot · inspect · press · set-value · focus · show-menu ·
  window-{focus,resize,move,close,minimize} · wait`.
- **Out of scope — the agents keep these regardless** (xa11y is a11y-tree only):
  the agent process + HTTP envelope, and the non-a11y endpoints `/exec`,
  `/upload`, `/download`, `/shutdown`, `/health`.
- **Out of scope — the host CLI** (`cli-rs/`): xa11y has no VNC/RFB, no OCR, no
  VM lifecycle/golden. It cannot touch the RFB client, OCR pipeline, QEMU/tart
  lifecycle, or golden creation. (Why this is a standalone grove and not a leaf
  on `port-swift-cli-to-rust`.)

## Done when

A replace/augment/reject decision exists **per platform**, with licensing,
maintenance, and maturity assessed, recorded as an ADR (and/or follow-up work
leaves). A "reject" outcome is a complete, valid result.

## Decomposition

Grown during the 010 grilling (decision tree in `010-plan.md` running log):

- `020-research-xa11y-maturity-and-coverage` — gates-first (4 maturity/licensing
  gates, short-circuit on failure) then a three-tier fidelity rubric scored per
  platform; produces three replace/augment/reject verdicts + roll-up, cited.
- `030-prototype-macos-snapshot-parity` — **gated on 020**: diff xa11y's
  snapshot vs. the live Swift agent on one macOS app for attribute parity;
  retires unstarted if research rejects.
- `040-decide-and-record` — synthesis: per-platform verdict + roll-up into an
  ADR (the Done-when); grows follow-up work leaves only if not reject-everywhere.

**Roll-up rule:** "replace" is only worth committing on a majority of platforms
— a single-platform replace keeps three agent languages *and* adds a dependency.

## Pointers

- In-VM agents: `agents/{linux,macos,windows}/`; routes:
  `agents/windows/AccessibilityEndpoints.cs`,
  `agents/macos/Sources/testanyware-agent/AgentServer.swift`.
- Host-CLI glossary (why host CLI is out of scope): `CONTEXT.md` → *In-VM agent*.
- Related groves as potential consumers/homes: vision-pipeline, ocr-accuracy.

## Notes

Seeded from a cross-grove inbox observation captured 2026-06-07 from the
`port-swift-cli-to-rust` grove (user decision: standalone grove, sequenced after
the `215-docker-host-unification` spike).
