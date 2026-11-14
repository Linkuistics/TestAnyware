# complete-worker-scheduler-k68


## Goal

Independently contest complete-worker-scheduler-k64's native SIGCHLD auto-reap admission boundary before the remaining scheduler implementation relies on it.

**Reviews:** complete-worker-scheduler-k64



## Context

Read the producer's committed diff and experiments/exec-reliability/evidence/native-auto-reap-k64/report.md, including the frozen RED, both repair attempts and both audit attempts. Scope is inspection._native_sigchld, the existing admission callers, worker_capture._admission, the real native-state controls and their measurement claims. The full aggregate charter remains in complete-worker-scheduler-k67; no aggregate scheduler acceptance is claimed here.

## Done when

- Re-derive the Darwin public sigaction ABI and SA_NOCLDWAIT semantics from the pinned manifests and primary sources. Challenge default/nondefault handler and flag combinations, observation failures and classification priority. Verify that a rejected state reaches controller acquisition and both existing worker/helper fork boundaries and that a contradictory ready receipt still fails.
- Inspect real native-state installation/restoration, the RED safety guard and the real-worker helper-refusal wrapper. Separate kernel actions from synthetic publication bodies. Challenge whether omission of the flag check and refusal of the new decoder state are caught; do not create an unsafe auto-reaped child merely to prove refusal. Use bounded ordinary checks only as needed for specific claims.
- Reconcile each retained sample's exact case inventory and expected failures, source/tool/bound provenance, failed first repair and failed audit. Challenge the independent auditor's counting and identity oracle. Preserve all evidence bytes; any new measurement gets a fresh declared directory. Do not turn later PID absence, worker reap or group membership into helper identity/reap proof.
- Verify the original aggregate charter is still fully owned by k67 and every ancestor remains open. No guest, installed-app, privilege or unrelated-process operations; ten-clone acceptance stays 0/10 unavailable and stalled storage remains separate.
- Record findings against the committed artifact with file/line evidence and concrete impact. Create an integrate-review-impl leaf before k67 only if findings warrant reconciliation; do not create an empty integration. The final aggregate bounded-inspection review before k21 remains k67's obligation.

## Notes

This is the required separate adversarial review of k64, not a second implementation session or permission to absorb the full scheduler. The producer's native-state snapshot does not prevent a host changing signal/thread state afterward. Its controls create no helper; RED worker receipts intentionally retain unknown creation after their guard fails.
