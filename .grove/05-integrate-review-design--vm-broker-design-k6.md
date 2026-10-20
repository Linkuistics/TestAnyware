# vm-broker-design-k6

**Integrates:** vm-broker-design-k5

## Goal

Triage the findings of `vm-broker-design-k5` against the agreed design and the
owner's requirements. Apply the findings that hold before planning uses the
design.

## Context

- Read the findings from the committed task record of `vm-broker-design-k5`.
  Find its commit by that handle; do not treat its proposed corrections as
  requirements.
- The producer is `vm-broker-design-k3`. Its decision log and `plan-k1` hold
  the owner's choices and their rationale.
- The artifact is `docs/specs/testanyware-app.md` and the ADRs it cites.

## Done when

- Each review finding has an evidenced disposition: apply, reject, or record
  an explicit accepted trade-off.
- Accepted changes are reconciled across the design, its ADRs and their
  citations, with appropriate verification and any necessary owner decision.
- `vm-broker-increments-k4` can plan from the resulting coherent design.

## Notes

Keep this a design integration. Implementation and release work belong to the
increments that planning cuts. The owner asked for focus and small increments.
