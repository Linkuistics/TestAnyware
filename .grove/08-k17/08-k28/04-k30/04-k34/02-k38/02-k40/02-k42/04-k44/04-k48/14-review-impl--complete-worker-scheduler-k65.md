# complete-worker-scheduler-k65

**Reviews:** complete-worker-scheduler-k62

## Goal

Independently challenge the raw helper fork/reset/exec and local exact-child ownership shipped by k62. Re-derive the boundary from the committed producer artifact and current source before k64 relies on it.

## Context

Read k62, k48 and the aggregate spec, then experiments/exec-reliability/evidence/raw-helper-fork-k62/report.md. The final run is green03; red01, green01, green02 and stdio-red01 are intentionally retained. Their source snapshots match their subject hashes. The change replaces inspection._HelperProcess's Popen subclass with a plain owner and explicit _spawn_helper/_helper_child; transport test seams changed, worker_capture runtime did not. This is a bounded runtime increment, not completion of k64 or any ancestor.

## Done when

- Trace actual admission, fork, signal mask, child reset/exec/exit, descriptor creation and closure, owner registration/recovery, exact-child reap/KILL authority and residual lifetime across success and all material failures. Inspect waits, destructors and concurrency boundaries in inspection.py, worker_capture.py and controller.py; distinguish production behavior from fixture reconciliation. Challenge ordering and closed-stdio behavior, including actual stdin inheritance through exec and parent failure after a known fork.
- Inspect actual child observations and receipts for inherited handlers/exit hooks, wakeup and evidence FDs, pending real signals, reset/exec failure, normal/slow/overflow capture, and terminal transport receipts. Fork errno, parent mask restoration and simulated wait non-observation are labeled adapters: do not promote them to real resource-exhaustion or unreapability proof.
- Verify frozen run subjects, source snapshots, checker budgets/results and evidence integrity. Reconcile the 50,053-file prior disk inventory with tracked and ignored evidence as applicable; no commit-only claim of ignored-file preservation. Retain newly produced verification evidence without altering earlier runs.
- Check status-70 ambiguity and unchanged limits are explicit: no child-error channel, persistent worker, gated publication, authenticated helper identity, controller-owned helper reap acknowledgement or shared scheduler. Confirm k64 preserves the entire original contract and all aggregate ancestors stay open; ten-clone acceptance remains 0/10 unavailable and privileged controls unavailable without authorized prerequisites.
- Report actionable findings with exact paths/lines and evidence in this review leaf. Cut integrate-review-impl before k64 only if findings earn it; no empty integration. Do not implement the full scheduler here or acquire privileges/change guests, applications or unrelated processes.

## Notes

Provider evidence was absent in the finishing producer's environment. No Creator is fabricated; caller review policy may require the owner's declaration separately.
