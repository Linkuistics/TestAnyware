# positive-worker-pid-k59

**Reviews:** complete-worker-scheduler-k55

## Goal

Independently challenge the worker helper PID validation increment, its executable controls and retained evidence before the remaining complete runtime producer relies on it.

## Context

Read the producer commit by stable handle, its narrowed task and the applicable ancestor contracts. The artifact is `experiments/exec-reliability/worker_capture.py`, its hostile transport controls in `experiments/exec-reliability/test_worker_capture.py`, the new `k55-*` evidence and the k55 section of `docs/experiments/exec-reliability-diagnostic-foundation.md`.

The full persistent-worker, gated-publication, independently owned helper and shared-scheduler contract remains live in complete-worker-scheduler-k58. This review owns only the delivered PID-validation boundary and the honesty of that decomposition. It does not replace the final whole-runtime review of bounded-inspection-k30 required before retained-evidence-k21.

## Done when

- Re-derive whether nonpositive helper PIDs are rejected in the actual fork/channel/decoder path, while missing identity on creation failure and legitimate positive identities retain their intended behavior. Inspect the integer/type, timing and creation-coupling checks that precede the new condition and the failure/shutdown path that follows it.
- Challenge whether each new hostile publication isolates its intended validation rather than failing an earlier guard. Check parent dispatch/receipt witnesses, matching creation stamps, raw values and frame bytes, exact rejection reasons, output disposal, unknown-creation residual, worker identity/reap and original deadline. Determine whether the retained RED run and final run establish the claimed regression and compatibility.
- Trace the audit's subject inventory, frozen source copies and their timing, before/after digests, prior-evidence manifest, per-case results, positive process-selector inventory, simulated-selector exclusions and independent oracle controls. Inspect scratch inventory, path bounds and retained removal evidence. Retain every failed attempt; distinguish observed later absence from original helper creation/ownership/reap evidence.
- Check the report and task split against the artifacts: no authenticated helper identity, persistent worker, gated launch, shared scheduler, aggregate completion, guest/app/privilege or ten-clone acceptance claim may be inferred from this increment. Every original runtime/control requirement must retain a live owner in k58 and its ancestors.
- Report findings with exact paths/lines and evidence, without fixing producer artifacts in the review. Insert an integrate-review-impl immediately before the first later live sibling only if findings earn reconciliation; create no empty integration placeholder.

## Notes

Use the current Grove review skill and its provider-evidence rules. No Creator line was supplied by this producer because its inherited environment has no GROVE_PROVIDER_EVIDENCE; do not invent a provider receipt.
