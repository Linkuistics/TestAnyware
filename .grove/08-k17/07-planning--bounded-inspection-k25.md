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

## Implementation handoff from bounded-inspection-k27

Review k26 is reconciled by k27. Read the seven dispositions in k27's commit
and the current spec. Group termination performs no query during grace and
attempts group KILL at T. PID-only KILL remains conditional on leader exit.
Each call gets at most one final query, with its explicit late-exit and
still-exiting-member failure limits. Helper identity publication precedes exec
without a per-helper controller ACK. Worker readiness still has an ACK.

The implementation must replace each private wait loop with steps in one
controller scheduler. This includes `_wait`, `_terminate`, the leader's
`Popen.wait`, resource teardown, channel handling and worker cleanup. Preserve
immutable call/plan/resource declarations and independent cleanup/disposal
ordering. Move signal 0 into the controller. Add worker lifecycle, bounded
framing, epoch validation and residual records together. Keep old receipts
readable, and extend the explicit self-check source/hash/type-check lists.

Implement the fork preconditions and inherited-state reset as observable
contracts. Freeze hold points and helper selection at worker bootstrap, with
ordinary conditions using the same source and disabled holds. Declare every
bootstrap, worker-failure and orderly-close bound before a run freezes. A
worker already retiring cannot restart its cleanup clock during teardown.

Move the subprocess-run stubs for foreign groups, usage errors, timeout and
missing leaders to adapter value cases. Keep the real process controls for
scope, reap, live members, checker rejection and exact-path resource audits.
Add the late-grace leader exit, single-final-query limits, inherited host state,
and worker timers during an open teardown command. Distinguish explicit fork
failure from an unknown creation attempt.

The deterministic KILL/wait control selects an owned slow helper. Actual KILL
of setuid ps is a separate timing-sensitive control. The UID-based EPERM fixture
needs a separately authorised privileged owner for creation and cleanup.
An unavailable receipt must identify the prerequisite and every unreached
operation. Neither an injected errno nor a different helper satisfies those
real-path controls.

Require review to trace helper creation, result drains, waits and shutdown
hooks through the actual runtime. No such path may block the safety owner or
act on a released workload identity. If the runtime defeats the contract,
externalise that precise counterexample to design before claiming a guarantee.
A longer timeout, replacement worker, extra thread or wider signal scope does
not repair it. k21 retains stream/result identities and surviving-writer checks.
k18 owns storage isolation and must preserve the scheduler and sole signal owner.

## Scope

This planning leaf exists because the design kind owns the contract and planning
owns implementation decomposition. Do not re-interview the already agreed root
seams or treat the design as an implementation pass. If runtime evidence defeats
the ownership design, externalize the precise redesign ahead of its consumer.
