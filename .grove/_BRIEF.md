# TestAnyware.gh-issue-12 — brief

## Goal

Make the `input` commands take effect in a macOS 27 guest again, as GitHub issue
12 asks. Two live checks prove it, and every release runs them. This grove also
removes the macOS guest versions older than 27. One release carries all of it.

## Done when

- Typed text reaches the focused field of a macOS 27 guest. A wheel scroll and a
  drag of the scroll bar move a view. The live checks show it on the installed
  release.
- A host-side test at the agreed seam guards the fault.
- Golden creation for macOS accepts macOS 27 or a later major only. No document
  offers an older macOS guest.
- The release is published, and this Mac runs it. The spec testanyware-app lists
  the input checks among the live checks of a release.

## Decomposition

- `input-live-k2` adds the live checks and reproduces the fault. It comes first,
  because its control shows where the fault lies.
- `input-delivery-k3` fixes the fault at its cause, with a host-side test.
- `macos-guest-versions-k4` removes the older macOS guests. It comes after the
  fix, so that the urgent part lands first.
- `release-k5` publishes the release that carries the three leaves before it.

## Pointers

- Issue 12: <https://github.com/Linkuistics/TestAnyware/issues/12>.
- The decisions of this grove are in the running log of `plan-k1`.
- ADRs: ADR-0004 (each command opens its own guest connections) and ADR-0018
  (the VNC keeper).
- Spec: `docs/specs/testanyware-app.md`. Its section "Guest traffic on a macOS
  host" states how a stream ends and when the app resets the guest connection.
  Its section on test seams lists the live checks of a release.
- Glossary terms in play: **Guest stream**, **Host-side framebuffer**, **VNC
  keeper**, **TestAnyware app**, **Golden image**, **Base VM**.

## Test seams

The human agreed these seams. Each code leaf of this grove tests through one of
them.

- **The live checks** are the acceptance seam. They drive a real macOS 27 guest
  through the app, as Koine does. They send `input click`, `input type`, `input
  scroll` and `input drag`. They read each effect through `agent snapshot`: the
  value of a field, or the position of one element. Every release runs them on
  the bundle of its candidate.
- **The routing seam** of `task app:routing-test` is the regression guard of the
  fault. It runs the real CLI and the real app executable against a fake RFB
  server. The fake gains a mode that copies what the live evidence shows tart
  does, so that the new test fails before the fix. If the cause lies elsewhere,
  the fix uses the highest existing seam at that point.
- **The existing CLI tests** of `task cli:test` cover the removal. They hold the
  unit tests of the version selector and the contract tests of `vm
  create-golden`.

## Notes

- Out of scope: Linux and Windows guests in the live checks, and macOS hosts.
  Hosts keep macOS 14 or later.
- The CLI surface stays compatible, apart from the removal that the human chose.
  Before a change that a client can read, sweep `~/Development` again (memory
  `testanyware-consumers`).
- Keep the golden `testanyware-golden-macos-tahoe` on this Mac. It is the data of
  the human. Tell the human that it can go.
