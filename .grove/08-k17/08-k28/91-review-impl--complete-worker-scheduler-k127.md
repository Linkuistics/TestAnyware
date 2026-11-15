# complete-worker-scheduler-k127

**Reviews:** complete-worker-scheduler-k124

## Goal

Independently challenge the committed shared bounded-turn driver and four
compatibility consumers. This review owns the narrowed k124 artifact, not the
aggregate shared scheduler or production teardown overlap.

## Context

Read the k124 producer, the k28 brief chain, docs/specs/diagnostic-process-inspection.md,
and experiments/exec-reliability/evidence/common-driver-k124/report.md. Runtime
changes are turns.py, controller.py and worker_capture.py under
experiments/exec-reliability; tests add test_turns.py, with manifest and Taskfile
integration. Frozen inputs and measurements include the setup-only RED failure,
missing-module RED, two initial exact-clock failures, five rejected behavioral
mutations, 285 final focused passes and four interleaved 873-case full/static
passes. Parent runtime consumers keep their original loops; identical test/driver
and manifest overlays run in both comparison conditions. Inspect per-item outcomes
and hashes rather than inferring causation from later passes.

The complete aggregate charter and historical log remain in
complete-worker-scheduler-k126. Existing capture transport loops still block safety
teardown; runtime wrappers currently each register one owner. The real composition
control registers multiple owners directly. No k28/k17 ancestor closed. The eventual
producer still owes the full aggregate review before k21.

## Questions to challenge

- Verify due safety priority, earliest wake ordering, equal-wake rotation,
  immediately due initial owners, terminal removal and capped sleeping only when
  no work is ready. Challenge clock movement/late dispatch and owner exceptions.
  Distinguish the trusted bounded-callback contract from validated transport input.
- Trace all four changed compatibility consumers. Check that immutable owner
  cutoffs, signal/probe authority, lifetime first reap, residual ownership and
  error propagation remain intact, including expired first turns and repeat close.
- Assess whether the real failed-worker/healthy-worker/workload composition and
  deterministic controls support the claimed shared-driver behavior. Check exact
  child identity, bounded checker cleanup, mutation sensitivity, subject hashes and
  all failed samples. Confirm the parent overlay isolates consumer replacement.
- Keep cooperative scheduling limits, sequential production owners, historical
  errno35/late-probe/late-WNOHANG samples and k126 requirements explicit. Do not
  accept this slice as full overlap, a repaired historical timing mechanism, or
  aggregate acceptance.

## Done when

Retain a findings report with source evidence and graph coverage limitations.
Use bounded frozen counterexamples only when needed; preserve prior evidence.
Follow review-impl's reconciliation route and insert integrate-review-impl before
k126 only if actionable findings earn it. Do not implement fixes in this review.
