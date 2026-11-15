# complete-worker-scheduler-k120

**Reviews:** complete-worker-scheduler-k116

## Goal

Independently challenge the completed k116 fork-failure EOF-order artifact
against its scoped criteria and the inherited process-inspection contract.
Derive the producer's commit from its stable handle and assess the artifact
from that boundary.

## Context

Read the producer task and its committed diff, the parent bounded-inspection
brief, docs/specs/diagnostic-process-inspection.md, and the F4 requirement in
experiments/exec-reliability/evidence/termination-review-k118/report.md.
The artifact is experiments/exec-reliability/evidence/fork-channel-k116/;
report.md and comparison-plan.json locate the measured inputs and outputs.
Changes also touch worker_capture.py, test_worker_capture.py, root Taskfile.yml
and docs/experiments/exec-reliability-diagnostic-foundation.md.

The entire inherited aggregate remains in complete-worker-scheduler-k119,
sequenced after this review. Check the split preserves its requirements; this
producer does not claim shared-scheduler completion. The first parent full run
also retains a named deferred-reap timing failure. Its disposition and limits
must be assessed from the actual evidence, without treating later passing
samples as a repair.

## Done when

- Independently assess whether recording cleanup EOF preserves no-child
  authority only for timely, valid, uncontradicted fork failure with both
  channels closed. Challenge late, partial, trailing, contradictory and
  open-channel paths, cutoff handling, read caps and ownership semantics.
- Assess the deterministic test's relation to real transport and reaping,
  whether its ordering adapter can conceal a failure, negative control
  adequacy and the strength of the mutation rejection. Separate injected
  faults from real observations and unexplained historical symptoms.
- Check the finite full-suite comparison, actual changed bytes, source/tool
  provenance, immutable earlier evidence, exact fixture cleanup outcomes,
  preserved failures and the report's causal and repeatability limits.
- Record findings with concrete source/evidence anchors, severity and reasons;
  distinguish demonstrated failures from uncertainties. Apply the review
  discipline's ordinary checks and bound any new experiments to owned local
  exact-identity fixtures with fresh evidence paths. Do not modify old samples,
  use guests or installed applications, or signal unrelated processes.
- Cut an integration leaf only if actionable findings warrant one, using this
  review's handle as the handoff. Keep k119 and aggregate acceptance live.

## Notes

This is an independent review of the committed producer, not a continuation of
its reasoning. Its body makes no correctness conclusion for the reviewer.
