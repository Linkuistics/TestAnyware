# diagnostic-evidence-k17

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
