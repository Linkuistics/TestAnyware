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

## Decisions (running log)

- 2026-11-15 k127 result: two findings, F1 actionable and F2 a test-sensitivity gap. An integrate-review-impl leaf (k128) was inserted before k126. This was a source-and-evidence review. No test, build or lint command ran. No retained evidence changed. No Creator line is written, because this session had no GROVE_PROVIDER_EVIDENCE.
  - F1 (real, low severity). The plan subject list at experiments/exec-reliability/controller.py:765-776 hashes controller.py, inspection.py, worker_capture.py, the interpreter and /bin/ps. It does not hash turns.py. Both controller.py (line 26) and worker_capture.py (line 22) now import turns.py. A change to turns.py therefore leaves `subjects_before` equal to `subjects_after`. Positive control: inspection.py and worker_capture.py are in the list, and history shows each was added with its runtime role (43e555b, 0189003). The k124 candidate commit did not add turns.py. The k124 report claims all measured subjects are unchanged, but that claim covers only the evidence runner's own list. Fix: add the entry, with a red control.
  - F2 (test sensitivity, not a runtime defect). All four consumers pass one Turn, so `safety=True` or `False` has no observable effect in production. The five mutants run `-k "shared_turns and not real"`, so the real composition test never runs under a mutant. No mutant removes the initial-due shortcut (`float("-inf")`). No mutant flips a consumer's `safety` flag. The driver rules are therefore proved only by injected-clock tests. Classify as a visible trade-off until k126 connects several owners.
  - Verified, no finding. (1) Driver policy. Ready safety work outranks ordinary work. Ties break by earliest wake, then by a growing sequence number. Initial owners are due at once. Terminal owners leave the queue. Sleep happens only when nothing is ready and is capped at 5 ms. (2) Consumers. `_WorkloadWait`, `_LeaderReap`, `_Termination` and `advance_close` keep their own immutable cutoffs. The first WNOHANG at an expired cutoff stays inside `_LeaderReap`. `begin_close` is idempotent through `self.closed` (worker_capture.py:457), and a repeat `advance_close` returns None once `shutdown_finished` is set. Exceptions propagate with no retry. The final snapshot and `termination_ok` in `_terminate` are unchanged (controller.py:615-639). (3) Measurement. The live sources match inputs-final by hash (turns f260e0b44d6f, controller 286c00cf8b67, worker_capture c01a9a151af4, test_turns 5428869fbd78, pyproject b4bbd5e79719). Each mutant differs from inputs-final at exactly one site. condition-audit.json shows 873 common identities in both pairs, no changed outcome, and the expected failing tests per mutant. All four full runs report four reaped checks and five fixture journals with fixtures_ok. controls-1 retains its two exact-clock failures.
  - Limits, not defects. The scheduler is cooperative. A callback that does not yield delays other owners, and an owner exception leaves the other owners unadvanced. k126 must decide how a multi-owner composition handles that. Production teardown is still sequential. The historical errno35 (k114), 4.944 ms late probe (k116) and 20.291 us late WNOHANG (k121) samples stay unexplained. The real composition control registers owners directly and does not show production concurrency.
  - Graph coverage. Generation 2026-11-15T02:19:52Z, full mode. turns.py and test_turns.py are not tracked. controller.py and worker_capture.py show metadata_changed. All four paths report no recorded issue. Coverage is best effort, so the source was read directly and no repository-wide claim is made.
  - Continuation. k126 keeps the full aggregate charter. k28 and k17 stay open. Guest acceptance stays 0/10 unavailable.
