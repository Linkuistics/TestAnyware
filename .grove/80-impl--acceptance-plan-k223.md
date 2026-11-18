# acceptance-plan-k223

## Goal

The acceptance controller has a trial plan and a checker for the root
acceptance inventory. The checker reads the new JSON record. The controller
self-check passes.

## Context

- Controller: `experiments/exec-reliability/controller.py` and its report
  `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- Spec: section "Validation route".
- Root brief: sections "Acceptance inventory and bounds" and "Constraints".
- Host CLI: the reviewed state of host-cli-control-k222, with the repairs of
  host-cli-control-k240. On the app route, a guest reset after a complete
  body reads as success. Bytes that reached the host still fail the framing
  check. The trial must not count that ordering as reset detection.

## Done when

- The trial plan runs every root case on each clone, in the root order. It
  keeps the abrupt CLI interruption separate from explicit cancellation.
- The checker reads JSON mode only. It compares the decoded `evidence` bytes
  with the root byte oracle.
- The checker applies the record validity rules of the spec. It accepts both
  leader orders and `leaderStatus` `unavailable`.
- The checker fails each of the four corruption controls of the spec:
  - an appended LF,
  - a flipped binary byte,
  - a missing terminal outcome,
  - a contradictory termination record.
- Each ceiling and bound of the root brief and the spec is a checker limit.
- The outer ledger disposes of each owned VM in `finally` within 45 s.
- `diagnostic:self-check` passes into a new `OUT` directory.
- No guest boots in this leaf.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

Leaf 68 runs the corruption controls against the real builds and freezes the
plan. A change to the plan after that freeze needs a new freeze.
