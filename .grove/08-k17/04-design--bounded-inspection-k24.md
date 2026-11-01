# bounded-inspection-k24

## Goal

Establish a process-inspection contract whose timeout cannot block owned-resource
teardown, then hand it to a concrete reviewed implementation before k21.

## Context

Read review termination-evidence-k22 in commit 0ee42bc4, integration
termination-evidence-k23 and the durable diagnostic foundation report's review
integration section. Start from k23-final-01, its new failure/mutation controls,
and corrected audits. Keep every earlier failed condition unchanged.

The current group sampling reserve is shorter than some observed ps calls.
More importantly, Python 3.12 subprocess.run timeout kills its helper then waits
without a timeout; creation is outside that timeout too. The setuid ps helper
can therefore block the controller's leader reap and finally disposal. A larger
reserve cannot establish the required guarantee. Storage isolation remains
diagnostic-disposal-k18; coordinate that ownership boundary rather than absorb it.

## Done when

- Define the smallest independently supervised inspection/ownership boundary,
  including helper creation, signal permission failure, uninterruptible wait,
  result handoff, leader identity pinning and an explicit observation interval.
  Do not release a group identity while any worker can still signal it.
- Specify how late, missing, contradictory and failed snapshots remain failures
  without suppressing safety actions. Preserve the shared grace/kill/reap
  ceilings and conditional-on-OS-scheduling limits. No retries, increased
  ceilings or false absence after failed inspection.
- Define deterministic controls for a real delayed inspector and the post-KILL
  wait path, plus genuine permission/unknown classification. Require retained
  failures and independent itemized cleanup of every known owned helper.
  Distinguish simulated unreapability from an actual uninterruptible kernel task.
- Deliver a spec/ADR only where warranted. Insert a concrete implementation
  successor before retained-evidence-k21, then insert review-design before that
  successor as the final producer act. Implementation must cut review-impl before
  k21; findings reconcile before that consumer. No empty integration is created.
- Update k17 and k21's inputs with what the implementation must preserve: a live
  final group fails even after expected normal/PID-only leader exit, residual
  members/remedy remain visible, and PID-only interruption never widens scope.

## Notes

No live guest, app quit, unsafe macOS process suite or acceptance claim. No
controller-crash recovery or arbitrary escaped-descendant containment promise.
The design/review/implementation chain must account for the open k17 process
inspection contract before that node closes. k18 still owns stalled storage.
