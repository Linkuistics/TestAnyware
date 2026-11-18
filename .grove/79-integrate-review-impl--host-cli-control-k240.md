# host-cli-control-k240

**Integrates:** host-cli-control-k239

## Goal

Triage the committed findings of host-cli-control-k239 against the current
Host CLI implementation and its agreed lifecycle and evidence-transfer
contract. Apply the findings that meet the acceptance and material-risk
threshold before acceptance-plan-k223 consumes this implementation.

## Context

- Review: host-cli-control-k239. Read its findings from that review's commit,
  not from a restatement in this task.
- Producer: host-cli-control-k222, commit
  `282ee507f4036ae74fc41ab959cf9b66f86fd91d`.
- Contract: `docs/specs/guest-command-lifecycle.md`, ADR-0025, ADR-0026,
  and the producer's Done when and notes.
- The review records its direct-source scope and unavailable graph checks.
  Recheck each finding against source and the actual supported route.
- Reuse `Taskfile.yml` and the existing client/CLI/app test seams. Required
  verification remains cli:test, app:routing-test, cli:clippy and cli:fmt,
  plus the concrete affected controls. Invoke project tasks through mise.

## Done when

- Every review finding has a source-verified disposition under the
  integration family's acceptance and material-risk threshold.
- Valid actionable findings are repaired and verified at their responsible
  boundary, with controls that detect the stated wrong outcomes.
- Accepted trade-offs and rejected findings name their evidence and the
  agreed contract; optional polish does not widen this increment.
- Existing text output, JSON fields, exit codes, routing, cancellation and
  download contracts remain accounted for. Record actual verification
  results and preserve adverse evidence.
- acceptance-plan-k223 has a concrete reviewed implementation handoff;
  the separate frozen acceptance trial remains required.

## Notes

This is a flat continuation under the root because it shares the existing
Host CLI and acceptance contract. It precedes acceptance-plan-k223 so that
intervening work cannot silently move the review's source coordinates.
Do not patch the global CLI or app, replace a golden, or operate on unrelated
guests. Use private builds and explicitly owned test resources only.
