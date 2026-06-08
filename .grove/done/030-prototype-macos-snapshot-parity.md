# 030-prototype-macos-snapshot-parity

**Kind:** work (prototype leaf)

## ⛔ Gate

**Do not start until `020-research` clears all four Q4 gates.** If 020's note
records a global-gate failure or a macOS reject, **retire this leaf unstarted**
(`grove-llm leaf-retire`) — the prototype has nothing to prove.

## Goal

The minimal, falsifiable proof that xa11y can *replace* macOS a11y snapshotting:
run xa11y against one real app, run the live Swift agent's `/snapshot` against
the same app, and **diff the two trees** for attribute parity.

macOS first because AXUIElement is the most coherent native framework — where
"replace" is most likely true and a fundamental flaw surfaces earliest — and we
have a cheap golden macOS VM (clone+start) plus the live Swift agent to diff
against.

## Method

1. Clone+start the golden macOS VM; confirm the Swift agent is live.
2. Pick one real app with a non-trivial tree (e.g. System Settings or a bundled
   test app — pick something stable across runs).
3. Capture the Swift agent's `/snapshot` JSON for that app's frontmost window.
4. Drive xa11y (Rust binding) against the same app/window; serialize its tree.
5. **Diff:** do the roles, names, geometry (frame), and hierarchy line up
   attribute-for-attribute with what the host CLI's selectors consume? Record
   every divergence.

## Done when

A short findings note records: the app/window tested, the two trees (or a
diff), and a verdict — **parity** (replace stays live on macOS), **gap closable
by a translation layer** (replace→augment), or **fundamental mismatch**
(macOS reject). Feed the verdict to `040`.

## Context

Settled in `010-plan.md` Q5. This is *not* a full agent rewrite — `snapshot`
attribute-parity is the single highest-information probe (Q3 identified it as
the most likely place a confident "replace" degrades to "augment"). Resist
scope-creeping into actions/window-geometry here; that's only worth prototyping
if parity holds.

## Notes

Reference the host CLI's expected snapshot JSON shape (what attributes its
selectors need) — that's the parity target, not xa11y's native richness.
