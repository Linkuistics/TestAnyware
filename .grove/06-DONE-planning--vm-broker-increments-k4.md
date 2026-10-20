# vm-broker-increments-k4

## Goal

Find the smallest independently useful working increments of the agreed design
of the TestAnyware app. Order them by dependency, and grow the tree. A release leaf
ends the work.

## Context

- `docs/specs/testanyware-app.md` and the ADRs it cites.
- The decision log of `plan-k1`.
- The decision log of `vm-broker-design-k6`. It records the owner's choice on
  upgrades and each correction of the reviewed design.

## Done when

- The tree holds the increments in dependency order. Each increment leaves the
  product working, with a behaviour that a test or a live check proves.
- A release leaf ends the work and publishes a tagged version to
  `linkuistics/taps`. Before it publishes, the herdr acceptance run passes. The
  live check of an upgrade also passes: the app keeps its signing identity and
  its one Local Network row.

## Notes

- The human asked for focus: small increments, and no rabbit holes.
- Agents under herdr are blocked today. An increment that unblocks them early is
  worth more than an increment that completes a layer.

## Decisions (running log)

**The tree has nine increments, ordered to unblock herdr early.** The order is
`agent-client-hyper-k7`, `app-streams-k8`, `app-bundle-k10`, `app-lifecycle-k12`,
`app-adoption-k14`, `app-version-skew-k16`, `app-configuration-k17`,
`app-diagnostics-k19` and `app-release-k21`. After `app-bundle-k10`, an agent
under herdr drives a guest that a person started. After `app-lifecycle-k12`, it
also starts and stops the guest. Adoption, skew, configuration, diagnostics and
the release follow, because none of them blocks an agent.

**Routing is opt-in until the bundle exists.** `TESTANYWARE_APP_SOCKET` turns
routing on in `app-streams-k8`. The leaf `app-default-routing-k26` flips the
default on macOS, after the signed app exists. Every handoff before it leaves the
direct path as the default, so the product keeps working. The existing macOS
tests that use loopback endpoints move to the harness in that leaf.

**Lifecycle commands move one at a time.** `vm list`, `vm start` and
`vm stop` with `vm delete` each get a leaf. They share the state directory, so a
half-moved set stays consistent.

**Configuration comes late.** The app needs only its built-in tool search to
serve herdr. The directory handshake lands in `app-protocol-core-k9`, and its
remedy names the variable until `config-in-app-k33` adds the file.

**Two single-leaf increments have no node.** They are `agent-client-hyper-k7`
and `app-version-skew-k16`. A node would add a brief with no second leaf.

**A `review-planning` leaf stands before the first increment.** Others build on
this decomposition for months. The leaf `vm-broker-increments-k38` reads it with
fresh context.
