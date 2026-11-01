# bounded-inspection-k25

## Goal

Turn the reviewed bounded-inspection contract into the smallest independently
verifiable implementation continuation before retained-evidence-k21.

## Context

Read producer bounded-inspection-k24, its design review and any reconciliation.
The durable contract is `docs/specs/diagnostic-process-inspection.md`; source
evidence and selected visual views are linked from the foundation report's
bounded-inspection-design section. k23-final-01 remains the delivered source
condition until the new implementation lands.

## Done when

- Insert concrete implementation work before k21. Prefer one vertical increment
  that replaces synchronous group inspection end to end through `execute`,
  preserves the existing call/resource declarations, adds worker ownership and
  epoch validation, and retains the required failure controls and helper audit.
  If too large, decompose only into increments that demonstrate useful behavior
  independently; no partial increment may claim the complete disposal guarantee.
- The implementation charter explicitly owns `controller.py`, any narrow
  inspection module, the local peer fixtures, `test_controller.py`, the explicit
  self-check subjects/type-check configuration, Taskfile recipes and the durable
  diagnostic handoff. It preserves all existing evidence directories unchanged.
- Require real creation/delay and post-KILL wait controls, genuine EPERM and
  unknown classification, bounded framing/epoch rejection, early-reap/scope
  mutations, independent itemized cleanup and before/after hashes. An unavailable
  real permission control stays unavailable and prevents a full guarantee claim.
- Preserve live-final-group failure and residual remedy after normal/PID-only
  leader exit; never widen PID-only signals. Keep helper/worker reaps distinct
  from workload reap. No worker replacement, query retry or increased ceiling.
- Require the implementation producer to insert its specific review-impl before
  k21 once the artifact exists; actionable findings reconcile before k21.
  No empty integration is created. Update k17, k21 and k18 inputs with concrete
  handles. k17 cannot close until reviewed implementation and retained evidence
  satisfy the node's entire contract.
- Keep stalled evidence storage with k18. The inspection handoff is bounded and
  in memory; persistence must not become an acknowledgement needed for safety.
  No live guest, installed-app change, unsafe process suite or acceptance claim.

## Notes

This planning leaf exists because the design kind owns the contract and planning
owns implementation decomposition. Do not re-interview the already agreed root
seams or treat the design as an implementation pass. If runtime evidence defeats
the ownership design, externalize the precise redesign ahead of its consumer.
