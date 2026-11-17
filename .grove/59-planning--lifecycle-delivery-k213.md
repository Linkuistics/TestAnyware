# lifecycle-delivery-k213


## Goal

Plan the delivery of the reviewed guest command lifecycle contract. Cut the
implementation, preparation, acceptance and handoff leaves that take issue #14
to the frozen five-clone-per-OS trial.

## Context

The producer is lifecycle-evidence-k8. Its artifacts are these:

- `docs/specs/guest-command-lifecycle.md`. The sections "Implementation
  candidates", "Test seams and required controls", "Bounds" and "Validation
  route" are the planning input.
- ADR-0023 to ADR-0026 in `docs/adr/`.
- The views in `docs/design/guest-command-lifecycle/`.

The review is lifecycle-evidence-k214. Read its findings first. Then read any
reconciliation leaf that it inserted. Do not rely on a part of the design that
a finding contradicts until a reconciliation settles it.

The root brief `.grove/_BRIEF.md` sets the acceptance cases, byte oracles and
ceilings. It also sets the continuation rules for this planner.

## Done when

- Cut the implementation leaves from candidates A to D in the spec. Keep each
  schema with its only useful reader. Candidate A is useful only with B or C,
  because it reads their record.
- Insert a `review-impl` leaf after each implementation leaf. Cut no empty
  review leaves.
- Add a Linux agent test task, because the spec's controls need one and it does
  not exist yet.
- Cut the preparation leaves. They build isolated production test builds, run
  the controller self-check and check the four corruption controls. They also
  declare the boot, slot and export bounds and freeze every subject, ceiling
  and bound before any measurement.
- Cut the acceptance leaf for five fresh macOS clones, then five fresh Linux
  clones, in sequence through the app. Keep every root case, byte oracle and
  ceiling.
- Cut the durable handoff and resource audit leaf.
- If the Host CLI or agent changes ship with the next release, cut a release
  leaf in this grove.
- Cut a `review-planning` leaf ahead of the first leaf that the plan adds.

## Notes

Many consumers use only the Host CLI, so its surface must stay compatible. Do
not patch the global CLI or app. Do not replace a golden. New goldens use
`TESTANYWARE_AGENT_BIN_OVERRIDE`. Durable jobs, leases, pooling, placement and
replacement backends stay outside issue #14.
