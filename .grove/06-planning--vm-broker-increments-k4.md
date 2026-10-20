# vm-broker-increments-k4

## Goal

Find the smallest independently useful working increments of the agreed design
of the TestAnyware app. Order them by dependency, and grow the tree. A release leaf
ends the work.

## Context

- `docs/specs/testanyware-app.md` and the ADRs it cites.
- The decision log of `plan-k1`.

## Done when

- The tree holds the increments in dependency order. Each increment leaves the
  product working, with a behaviour that a test or a live check proves.
- A release leaf ends the work and publishes a tagged version to
  `linkuistics/taps`. Before it publishes, the herdr acceptance run passes, and
  the grant survives an upgrade of the app.

## Notes

- The human asked for focus: small increments, and no rabbit holes.
- Agents under herdr are blocked today. An increment that unblocks them early is
  worth more than an increment that completes a layer.
