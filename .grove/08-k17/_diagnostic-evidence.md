# diagnostic-evidence-k17 — brief

## Goal

Make the local controller's termination and retained-evidence claims checkable
before live route preparation or causal measurement uses them.

## Context

Read review `diagnostic-foundation-k15` in commit `4bb47c05` and integration
`diagnostic-foundation-k16`. The unresolved scope is F4, F5 and the remaining F6
controls. F7's historical audit is closed by k16's itemised audits. The current handoff is
`docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- Leader exit after TERM has a monotonic observation and PID-only KILL is
  conditional on need. Establish a safe group-inspection contract while pinning
  the leader. Do not signal a group after reaping permits identity reuse.
  Distinguish zombie-only EPERM, genuine signal permission failure, surviving
  members and failed inspection. Keep declared grace/kill/reap totals.
- Correct the local fork fixture's signal-handler wait-lock deadlock. Add
  opposed fixtures that distinguish PID-only CLI interruption from a group
  signal, and normal leader exit with a surviving writer. Positively inspect
  and clean each owned known child. Do not infer arbitrary descendant absence.
- Pin raw stream lengths/digests and result identity, demonstrate detection of
  late changes, and state the exact observation interval. A snapshot is not
  immutable publication. Each meaningful guarantee has a control shown failing
  against its broken condition, including wrong-stdout acceptance and wrong
  signal scope. Remove tautological controller-survival assertions.
- Reuse k16's itemised historical audit. Audit every new owned local resource
  and known child this producer creates. Preserve original receipts. Verify
  exact paths and process identities. A ps usage error cannot count as absence.
- Retain per-test/checker verdicts with source hashes in new directories, reuse
  the Taskfile runner, update the handoff and resource audit. No live guest,
  app quit, unsafe macOS process suite or acceptance claim.

## Notes

The stalled-filesystem architecture belongs to `diagnostic-disposal-k18`.
This producer cuts its specific `review-impl` ahead of the first live consumer
as its last work act. Integration is created only if that review warrants it.
Decompose if the independently demonstrable evidence contracts exceed a leaf.

<!-- tc: off because the decomposition records earlier producer and review conditions -->

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
It selects a prestarted read-only inspection worker with published helper
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

Review k26 is reconciled by bounded-inspection-k27. The contract now omits
pre-escalation queries and sends group KILL at T, with conditional PID-only
KILL unchanged. Helper publication precedes exec without a per-helper ACK.
The worker resets inherited host state. One controller scheduler serves every
wait, teardown action and worker timer. k25 owns that explicit restructuring,
the frozen bootstrap adapter and migration of existing inspection stubs.
The spec declares final-query limitations and every bootstrap/close bound.
Real setuid-ps KILL and privileged UID-based EPERM controls retain separate
unavailable prerequisites. k21 preserves these outcomes and k18 preserves the
scheduler while isolating storage. The delivered initial admission slice is
k32-final-01, following k29-final-01 and review integration k32. This partial
slice does not implement the worker/scheduler boundary: the synchronous
inspection limitation measured at k23-final-01 remains, with k30 and k21 open.

Planner bounded-inspection-k25 cuts bounded-inspection-k28 as one complete
implementation increment before retained-evidence-k21. It owns shared scheduling
through `execute`, the worker/helper lifecycle, bounded protocol, epoch checks,
all required controls, explicit runner/type-check subjects and the diagnostic
handoff. Splitting those mechanics would not establish the safety behaviour at
the caller. k28 must insert its concrete review-impl before k21 after code and
receipts exist; findings reconcile there before k21 runs. No empty integration
is planned. k17 remains open through that reviewed handoff and k21's retained
evidence, with missing real permission/setuid-ps controls still visible at its
whole-contract check. k18 separately consumes the reviewed scheduler and
immutable in-memory outcomes while isolating storage. Planning delivers no new
runtime guarantee or acceptance evidence.

<!-- tc: on -->

## Current structure

This node earns its shared context for termination and retained-file integrity: the
joint completion requires reviewed F4/F5/F6 controls before live diagnostics.
Bounded-inspection-k28 earns the worker/scheduler subgoal and its shared fault
inventory. Its sessions are now flat siblings.

Former continuation-node handles
k30/k34/k38/k40/k42/k44/k48 route through
[Grove brief history](../../docs/planning/grove-history/README.md). Their unfinished
contracts are promoted into k28. k72 precedes k71, with complete runtime
review/reconciliation before k21. Historical statements above about k30 or an
earlier synchronous source condition remain provenance, not acceptance of the
current runtime. k17 stays open through k28 and k21. Storage remains k18.

## Worker scheduling delivery and review boundary

The k146 producer connects failed worker retirement and close to production public teardown scheduling.
The [production report](../../experiments/exec-reliability/evidence/worker-overlap-k146/report.md) contains the runtime ownership trace, full control map and frozen comparisons.
Both candidate comparisons pass 949 cases, and the parent comparisons reject the eight new production overlap controls.
The source condition is the report's `full-candidate-inputs`, with unchanged runtime bytes in the finishing producer.

The k28 implementation review must precede k21 and any reliance on its complete contract.
It must trace every current owner, wait, signal entry point, deadline and residual obligation across actual `execute`.
The report records separate unavailable setuid-ps and real UID-denial receipts, plus unresolved historical timing and fixture failures.
Those limits remain part of k17's acceptance assessment even after the implementation producer closes.

k21 consumes immutable observations and residual inputs without treating worker reap as helper or workload reap.
k18 owns storage stalls and must preserve the scheduler's in-memory safety boundary.
The process scheduler alone certifies neither retained-file identity nor disposal through blocked evidence storage.
Guest acceptance remains 0/10 unavailable, and k17 remains open through review reconciliation and k21.

The complete implementation review is recorded in bounded-inspection-k147 beside this brief.
It inspected producer commit `33c7cee4`, the current runtime and callers, the full material control inventory and retained comparison artifacts, without rerunning tests, builds, linters or formatters.
No new actionable defect was established, so no integration leaf is required before k21.
Independent checks confirmed the exact full-run identities, rejecting mutations, sixteen overlap receipts, frozen runtime bytes and the 293,984-path preservation commitment.

This closes the implementation-review prerequisite only.
Historical timing failures and k145's interrupted live-group failure remain unexplained; passing later samples do not repair them.
Real setuid-ps KILL and genuine different-UID denial remain unavailable, preventing a full guarantee.
k21 must retain these limits and independent worker, helper, workload and observed-member obligations in result identity.
k17 remains open for retained-evidence work and its whole-contract assessment; k18 still owns blocked storage and guest acceptance remains 0/10 unavailable.

## Retained identity review and remaining acceptance

Retained-evidence-k148 inspected producer commit `14f1e91d` and its source,
recorded controls, full artifact anchors and resource audit without rerunning
verification workloads. No actionable defect was established, so no integration
leaf is required. Stream pins and the exact stdout oracle share one observed
byte sequence; result identity follows the full result without a cycle; later
verification rejects changed, missing and added files. The late-writer record
keeps termination failure and adds the subsequent stream rejection, with a
positive known-child absence observation. The review records one nonblocking
audit-category correction in its own findings. Snapshot, identity trust and
storage limits remain unchanged.

The whole-contract check still fails on unresolved historical timing/fixture
observations and missing real setuid-`ps` KILL and UID-denial controls. With no
other live child remaining, diagnostic-failure-controls-k149 owns the concrete
reproduction, repair and real-control attempts. Its handoff is evidence resolving
those conditions, not another aggregate inventory. Required privileged-owner
input keeps that task live rather than producing another unavailable completion.
k17 stays open; k18 owns stalled storage and guest acceptance stays
**0/10, unavailable**.

## Review integration and the open acceptance decision

Diagnostic-failure-controls-k151 applied the three k150 findings. The k151
evidence keeps the k149 receipts and replaces the k149 verdicts. The child audit
now reports "unknown" without a retained identity or a clean `ps` result. A
red/green control on real `ps` results shows the change.

Six historical cases have no demonstrated cause. The k145 interrupted group
printed its child PID, so the never-ready control does not explain it. Five
groups ran out a bound, and the runtime failed closed in each. No control
reproduces those stalls. The retained receipts carry no timestamp that could
separate the possible causes.

The k17 whole contract is therefore not met. Another inventory or control rerun
cannot close it from the retained data.

The human chose new instrumented evidence over accepting the six cases as
visible limits. Historical-failure-soak-k152 records the missing timestamps in
one bounded soak. It explains each recurrence, or it records the run count
without a recurrence. k17 stays open until k152 reports. k18 owns stalled
storage, and guest acceptance stays **0/10, unavailable**.

## Soak result and fixture cleanup

The k152 soak reproduced one case, the run 42 deadline, and five cases did not
recur. Worker-deadline-cleanup-k154 reproduced the run 42 leak with a stalled
signal and repaired the fixtures, so that case is a `fixture_defect`. The cause
of the run 42 observation gap is still unknown.

Worker-deadline-cleanup-k157 integrated the k156 review of k154. Its
`child-inventory.json` lists 486 direct children from the retained receipts.
It names a reaper only from a recorded fact, and two reapers stay unknown. It
replaces the k154 claim that `resource-audit.json` lists each child.

Five historical cases did not recur in the bounded soak and have no
demonstrated cause. The cause of the run 42 observation gap is unknown. On
2026-11-16 the human accepted both as visible limits and closed k17. The limits
are not guarantees. A recurrence of any of the five cases reopens its causal
question. k18 owns stalled storage, and guest acceptance stays
**0/10, unavailable**.
