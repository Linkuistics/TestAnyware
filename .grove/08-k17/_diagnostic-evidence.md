# diagnostic-evidence-k17 — brief

## Goal

Make the local controller's termination and retained-evidence claims checkable
before live route preparation or causal measurement uses them.

## Context

Read review `diagnostic-foundation-k15` in commit `4bb47c05` and integration
`diagnostic-foundation-k16`. The unresolved scope is F4, F5 and the remaining F6
controls. F7's historical audit is closed by k16's itemized audits. The current handoff is
`docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- Leader exit after TERM has a monotonic observation and PID-only KILL is
  conditional on need. Establish a safe group-inspection contract while pinning
  the leader; do not signal a group after reaping permits identity reuse.
  Distinguish zombie-only EPERM, genuine signal permission failure, surviving
  members and failed inspection. Keep declared grace/kill/reap totals.
- Correct the local fork fixture's signal-handler wait-lock deadlock. Add
  opposed fixtures that distinguish PID-only CLI interruption from a group
  signal, and normal leader exit with a surviving writer. Positively inspect
  and clean each owned known child; do not infer arbitrary descendant absence.
- Pin raw stream lengths/digests and result identity, demonstrate detection of
  late changes, and state the exact observation interval. A snapshot is not
  immutable publication. Each meaningful guarantee has a control shown failing
  against its broken condition, including wrong-stdout acceptance and wrong
  signal scope. Remove tautological controller-survival assertions.
- Reuse k16's itemized historical audit; audit every new owned local resource
  and known child this producer creates. Preserve original receipts. Verify
  exact paths and process identities; a ps usage error cannot count as absence.
- Retain per-test/checker verdicts with source hashes in new directories, reuse
  the Taskfile runner, update the handoff and resource audit. No live guest,
  app quit, unsafe macOS process suite or acceptance claim.

## Notes

The stalled-filesystem architecture belongs to `diagnostic-disposal-k18`.
This producer cuts its specific `review-impl` ahead of the first live consumer
as its last work act; integration is created only if that review warrants it.
Decompose if the independently demonstrable evidence contracts exceed a leaf.

## Decomposition

The termination and retained-file contracts have independent failure controls.
`termination-evidence-k20` delivers F4 and the process/signal-scope portion of
F6, its owned-resource audit and a concrete review. `retained-evidence-k21`
then delivers F5, the surviving-writer fixture, stream/result identities and
late-change controls. Both must be reviewed before k17 closes. F7's historical
audit remains k16's, unchanged. No live route or causal sample is permitted by
completion of only the first child.

Review k22 is reconciled by termination-evidence-k23. Live final group members
now fail termination even with an expected normal exit or PID interruption;
observed members and a revalidation remedy are retained without widening the
signal scope. New mutation controls and corrected exact-path audits are in the
durable handoff. Sampling latency and the helper's unbounded post-timeout wait
remain real issues: bounded-inspection design and reviewed implementation must
precede retained-evidence-k21. The present controller cannot promise independent
disposal while ps creation/kill-wait blocks. Storage isolation is still k18.

Bounded-inspection-k24 delivers
`docs/specs/diagnostic-process-inspection.md` and its linked visual document.
It selects a prestarted read-only inspection worker with registered helper
creation, bounded handoff and sticky observation failure. The controller alone
owns workload signals and reaping; a worker timeout must not defer either or
resource teardown. The design review precedes a bounded planning successor,
which must cut the concrete implementation and ensure review-impl/reconciliation
before k21. This follows the design kind's planning boundary. No implementation
guarantee is discharged by the spec. k17 remains open through that chain and k21.

The implementation preserves live-final-group failure after normal/PID-only
leader exit, visible residual members/remedies and unchanged PID-only scope.
Its controls must cover real delayed creation/inspection, actual KILL followed
by a controlled wait stall, genuine permission failure/unknown classification,
late/contradictory handoff, and independent itemized helper cleanup. Simulated
unreapability is explicitly distinguished from an actual kernel task. All old
conditions remain unchanged. k18 consumes the nonblocking in-memory handoff and
owns blocking evidence I/O; it must not introduce another workload signal owner.
