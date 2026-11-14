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

## Decisions (running log)

- Review of complete-worker-scheduler-k64 (commit 9e95da25) found no defect that needs reconciliation. No integrate-review-impl leaf is cut. k67 keeps the full aggregate charter.
- The sigaction layout in experiments/exec-reliability/inspection.py:62 matches the SDK header: handler pointer, 32-bit mask, integer flags. SA_NOCLDWAIT is 0x0020 in the SDK.
- experiments/exec-reliability/inspection.py:82 checks the flag only when the handler is null. A nonnull handler or SIG_IGN keeps its own label and is still refused at inspection.py:117. The priority is sound.
- Observation failures stay covered by the existing native_observation_error tests (test_inspection.py:557-619). The three existing callers (inspection.py:278, worker_capture.py:303, controller.py:618) all gate on the same receipt before any fork or acquisition.
- Decoder: worker_capture.py:140 accepts the new state and worker_capture.py:156 re-derives the reason. A ready receipt with SA_NOCLDWAIT still fails as "inconsistent worker admission".
- Omitting the flag check is caught. The guard at test_inspection.py:50 fails the RED test before any fork. The real kernel state is restored and asserted at test_inspection.py:53-60.
- Evidence: red01, green01 and green02 each hold 269 identical cases. Failures are 10, 1 and 0. The failed first repair (green01) and failed audit01 are disclosed and consistent with report.md.
- Minor, non-blocking notes. No test uses benign flags such as SA_RESTART or SA_NOCLDSTOP alone with the default handler. The audit02 PID check has no positive control for ps. docs/experiments/exec-reliability-diagnostic-foundation.md:616 is an older k32 section that omits the flag. The k64 section at line 1775 supersedes it.
- PID absence, worker reap and group membership were not used as helper identity or reap proof. No guest, privilege, installed-app or unrelated-process operation was run. Ten-clone acceptance stays 0/10 unavailable.
