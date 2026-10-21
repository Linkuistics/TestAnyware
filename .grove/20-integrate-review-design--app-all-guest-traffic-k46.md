# app-all-guest-traffic-k46

**Integrates:** app-all-guest-traffic-k43

## Goal

Triage the findings of `app-all-guest-traffic-k43` against the design produced by `app-all-guest-traffic-k42`. Apply the valid design corrections before `app-all-guest-traffic-increments-k45` cuts the implementation leaves. Record a reason for each finding that is rejected or accepted as a trade-off.

## Context

- Read the review's own committed findings by its stable handle `app-all-guest-traffic-k43`. Its evidence is anchored to producer commit `880100aa018707d9a96a217ba2a133b54966b9fb`; task-file paths can move, so resolve handles rather than assuming the old positions.
- The design is `docs/specs/testanyware-app.md` and its cited ADR set. The producer's decision log is `app-all-guest-traffic-k42`.
- The root brief states the current requirements. `non-macos-runtime-check-k40` records the owner's later instructions and the failure that prompted the design.
- The planning charter is `app-all-guest-traffic-increments-k45`. The release node is `app-release-k21`; `release-without-windows-host-k44` already owns the distribution narrowing and its related document corrections.

## Done when

- Every finding in `app-all-guest-traffic-k43` has a reasoned disposition based on the requirements and evidence, rather than being treated as an instruction to implement automatically.
- Valid design corrections are applied consistently to the spec, the affected ADRs and the planning charter, with any necessary glossary or brief correction.
- A question that needs the owner's decision is framed with the specific trade-off, a recommendation and its evidence. Substantial new design work is externalised ahead of planning under Grove's procedure.
- The resulting design and planning charter form a coherent handoff. This integration changes no production or test code; implementation and its verification belong to the later leaves.

## Notes

The review is inspection-only and ran no test, build, lint or format command. Its graph index could not start, so its source claims use direct reads and disclose that limit. Triage the review's findings from its commit; this leaf deliberately does not restate them as its own charter.
