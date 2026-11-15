# retained-evidence-k21

## Goal

Complete diagnostic-evidence-k17's retained-file contract (F5 and remaining F6)
after termination-evidence-k20 and its review are reconciled.

## Context

Read k17's node brief, k15's original findings and k16's historical audit.
Reuse the controller, Taskfile self-check runner and k20's process receipts.
Read k23's integration dispositions and delivered k23-final-01 plus corrected
audit 04 in the handoff. The bounded-inspection design/implementation/review
continuation runs first; use its delivered sources rather than treating k23's
conditional timing as a proved bound.

Read `docs/specs/diagnostic-process-inspection.md` and the implementation chosen
by its bounded planning continuation, after design and implementation reviews
are reconciled. Preserve observation intervals, sticky failed snapshots,
worker/helper identities and cleanup verdicts when pinning result identity.
Do not hash a late valid snapshot into an earlier failed result as a correction.
Inspection-unavailable safety actions remain uncertain even when their command
exit is zero. A surviving writer still fails termination and retains its
identity/remedy; the late-byte verdict is additional. PID-only interruption
never widens to a group signal. k18 still owns stalled storage.

Integration bounded-inspection-k27 reconciles the design review. Retain its
explicit final-query limits, fork-failure versus unknown-creation outcomes,
inherited-state controls and declared bootstrap/close bounds in result identity.
Missing privileged EPERM or actual setuid-ps KILL evidence remains unavailable.
A deterministic substitute helper does not fill either gap. Read k25's concrete
reviewed implementation handoff before adding retained-file guarantees.

The concrete implementation is bounded-inspection-k28. Its producer must insert
review-impl before this leaf once its artifact exists; consume k28 only after
that review and any reconciliation. Preserve its failed and unavailable real
controls, explicit bootstrap/worker-close phases, separate workload/worker/helper
reaps and unknown creation obligations in result identity. Do not replace their
verdicts with a later hash or a successful disposal command. k28 does not own
this leaf's surviving-writer and stream/result integrity work, and neither leaf
closes k17 while its whole-contract check still has unmet obligations.

## Done when

- Pin each raw stream's byte length and digest at an explicit monotonic
  observation interval, and independently pin result identity without a hash
  cycle. Exact stdout checks use the same observed bytes.
- Expose and demonstrate later verification that rejects changed/missing raw
  captures and changed/missing result files. State snapshot versus immutable
  publication limits, including intervening changes reverted before checks.
- Add a normal-leader-exit fixture with a surviving writer and prove that a
  late append invalidates the original receipt. Positively inspect and clean
  every known owned child; do not infer arbitrary descendant absence.
  Preserve k23's contract: that live group already fails termination and retains
  residual identities/remedy; the late-write check is an additional independent
  evidence verdict. Do not make the case pass by weakening termination.
- Show controls fail for wrong stdout, late writes and corrupted result
  identity; retain per-test verdicts with frozen source hashes in new evidence
  directories. Audit all resources this child creates, preserving older runs.
- Update the handoff and root limits; cut a specific review before the next
  live consumer. Close k17 only after the full node contract is accounted for.

## Notes

No live guest, app quit, unsafe process suite or acceptance claim. Blocking
storage isolation remains diagnostic-disposal-k18. Reuse k16's historical F7
audit rather than rerun or reinterpret its original uncertain receipts.
