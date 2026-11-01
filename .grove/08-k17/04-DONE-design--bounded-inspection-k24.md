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

## Decisions (running log)

- Use one prestarted inspection worker, separate from the controller that owns
  target signals and reaping. The worker and its ps children may read target
  identities but never signal them, including signal 0. Helper creation, exec,
  capture and post-KILL waiting stay behind this boundary. A worker deadline
  invalidates evidence without delaying target safety actions.
- Bootstrap the worker before resource acquisition. Register and acknowledge
  each helper before permitting exec; missing registration remains an unknown
  creation attempt. Keep all channels nonblocking and bounded. One failed
  worker is not replaced during a run; safety actions continue with inspection
  unavailable and uncertain outcomes retained.
- Preserve two-second grace and the shared two-second kill/drain/reap ceiling.
  The controller owns absolute deadlines and freezes observation epochs before
  reaping; it never waits for worker exit or a result at those deadlines.
  A reaping time allocation is scheduling policy, not a claim about ps latency.
- This local, reversible diagnostic design does not satisfy the ADR test. Its
  reusable ownership contract warrants a durable spec and linked visual views.
  Existing process/transport seams agreed in the root brief remain the test
  boundary. No production protocol or agent changes belong here.
- Grove's design skill reserves cutting implementation leaves for planning.
  The concrete continuation therefore passes through a bounded planning leaf
  before k21; it must create the specified implementation and its reviewed
  handoff there. This follows the kind contract while preserving k24's required
  design-review-before-implementation-before-k21 order.
- The spec and three editable views are written; bounded-inspection-k25 is the
  concrete planner before k21. k17, k21, k18 and the root brief now consume the
  ownership contract without treating it as shipped code. The source inspection
  used Tier 2 generation 2026-11-01T05:03:14Z, matching coverage metadata and
  exact snippets; spurious cross-language graph edges were not credited.
- Inline design review clarified that denied signal attempts also advance the
  observation epoch, healthy worker shutdown has the same bounded accounting,
  and preparation failures close an already started worker. Independent review
  belongs to the queued review-design, so no in-session reviewer is added.
- Browser checks found and corrected two Mermaid separator errors, then showed
  all three rendered views and a fresh deadline deep link at desktop width.
  The viewport capability did not change the measured 1227px viewport, so mobile
  rendering is unverified; no dark-mode override is exposed. Document links,
  manifest identities, source availability and changed markers pass a direct
  check with unchanged before/after subject hashes. Runtime reliability tests
  were not run for this design-only change.
- Final coverage reports stale metadata for the edited spec/task and parse
  gaps in the ownership Mermaid source and helper-start alternative line.
  Direct reads cover those exact files/ranges; browser rendering verifies the
  diagrams independently of the graph parser. No coverage-completeness claim
  follows. The final prose also makes bootstrap cleanup PID-only until worker
  session readiness is confirmed, and pre-escalation sampling grace-only.
- The reusable `design:inspection-preview` task served the manifest successfully.
  Both temporary servers (PIDs 57885 and 1242) were stopped by exact PID and
  their tool sessions returned after termination; both preview tabs were closed.
  No diagnostic workload, guest, golden, archive or TestAnyware app was acquired.
- The scheduled design review will challenge creation/exec races, signal and
  reap authority, nonblocking deadline handling, permission evidence, storage
  boundaries and the implementability of the controls. k17 stays live through
  that review, k25's implementation continuation and k21. No parent closes.
  Current provider evidence is absent, so no Creator line is invented.
