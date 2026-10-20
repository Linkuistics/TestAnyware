# vm-broker-increments-k39

**Integrates:** vm-broker-increments-k38

## Goal

Triage the five findings of `vm-broker-increments-k38` and correct the planned
subtree where they hold, before the first implementation increment runs.

## Context

- Read the committed review record of `vm-broker-increments-k38`. Its findings
  cite the producer commit `15985c4de06b9d3611c57ec7452a8b7dac587d1c` and paths
  before this integration was inserted. Resolve the stable handles for current
  paths; the insertion renumbered sibling entries without changing their bodies.
- The producer is `vm-broker-increments-k4`. The artifact is the root brief and
  the planned subtree from `agent-client-hyper-k7` to `release-publish-k37`.
- The agreed contract is `docs/specs/testanyware-app.md`, ADRs 0004, 0019,
  0020 and 0021, and ADR-0009 for non-macOS runtime verification.
- Findings concern safe legacy QEMU stop before signal guards, configuration
  activation across both processes, the invalid-config doctor path, preparation
  of release candidates before tag approval, and non-macOS runtime evidence.

## Done when

- Every finding has an evidenced disposition: apply, reject, or record an
  explicit accepted trade-off. Proposed corrections in the review are inputs
  to triage, not new requirements.
- Each revised leaf has a complete behaviour and an assigned check that can
  pass without waiting on a later sibling. The early herdr unblock stays early.
- Release preparation, acceptance checks, owner approval and publication have
  an executable dependency order, with candidate identity preserved.
- The root brief and affected node briefs agree with the revised ordering and
  ownership. The spec's requirements and existing compatibility obligations
  have assigned implementation and verification work.

## Notes

This is planning integration. Change task boundaries and their briefs, not
production or test code. The owner asked for small increments and no rabbit
holes. The review ran no executable checks, and its graph-index limitation is
recorded in its inspection limits.
