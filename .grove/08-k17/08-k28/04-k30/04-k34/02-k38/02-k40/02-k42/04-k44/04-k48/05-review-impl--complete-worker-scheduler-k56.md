# complete-worker-scheduler-k56

**Reviews:** complete-worker-scheduler-k52

## Goal

Independently challenge the shipped worker receipt timing boundary, its retained evidence and process ownership claims before complete-worker-scheduler-k55 relies on it.

## Context

- Producer complete-worker-scheduler-k52 is the timing slice. k55 retains the full original persistent-worker/gated-publication/shared-scheduler contract; this review does not close any ancestor or replace the final bounded-inspection review before k21.
- Runtime: experiments/exec-reliability/worker_capture.py, especially _decode and the real capture_helper call/receipt/cleanup boundary; experiments/exec-reliability/inspection.py for the child engine and HelperCapture semantics. Tests: experiments/exec-reliability/test_worker_capture.py.
- Retained conditions under experiments/exec-reliability/evidence/: k52-red-01 (12 failed/166 passed), k52-fixed-01 (2 failed/177 passed), k52-fixed-02 (179 passed), k52-inputs-01 source copies and k52-audit-01 frozen evidence/resource audit. Durable interpretation: docs/experiments/exec-reliability-diagnostic-foundation.md, k52 section.
- Current graph evidence is Tier 2, project Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14, generation 2026-11-14T10:32:27Z. _decode has the live capture_helper caller; final runtime/test/auditor coverage is metadata_match/no_recorded_issue (best effort only). Reconfirm freshness in this review; read flagged source ranges if coverage changes.

## Done when

- Challenge required timestamp types/finiteness/nullability, dispatch <= start <= finish <= parent receipt <= immutable query deadline, capture-cutoff bounds and optional creation/kill/reap causality. Check ordinary early exits, spawn failures and real timeout captures against those rules; determine whether valid receipts can be rejected or impossible receipts accepted.
- Follow actual fork/nonblocking read/frame/decode/publication paths. Challenge where dispatch and complete-channel receipt are witnessed, pre/post-decode deadline crossings, parent overwriting finish/deadline, rejected raw evidence, retained published_times, partial/duplicate/oversized/backpressured publication and failure/unknown-creation behavior. Distinguish the sixteen synthetic timing publications (no helper) from real helper controls and identify missing meaningful controls.
- Check wait/signal authority and exact worker/helper ownership across decode failure, ECHILD, early reap, close failure and deadline paths touched by this change. Do not infer helper death from worker reap or group KILL, and do not promote child-reported identity/reap to independent proof.
- Verify runner/checker/per-test results, source-copy and artifact hashes, timing payloads and the resource auditor's exact-identity oracle and scratch reconciliation. Assess report claims against retained samples; later absence does not repair unknown creation or historical missing identities. Keep old evidence bytes unchanged and retain any new experiment in a fresh condition.
- Record actionable findings with concrete source/evidence locations and checks performed. If findings require changes, insert integrate-review-impl complete-worker-scheduler before k55, **Integrates:** complete-worker-scheduler-k56; otherwise retire without an empty integration task. Preserve every remaining k55 and ancestor requirement.

## Notes

The producer's routine audit is evidence/resource verification, not this independent adversarial review. No guests, installed app modifications, privilege acquisition or unrelated-process cleanup. Historical acceptance remains 0/10 unavailable; stalled-storage and final whole-runtime controls remain separately owned.
