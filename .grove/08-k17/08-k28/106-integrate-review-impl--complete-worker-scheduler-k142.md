# complete-worker-scheduler-k142

**Integrates:** complete-worker-scheduler-k141

## Goal

Triage the findings of complete-worker-scheduler-k141 against the k137 expired-request control repair. Apply the real part narrowly. Leaf k140 keeps concurrent production owners, teardown overlap and every historical failure.

## Context

Read the k141 Decisions log (findings F1 to F3), then `experiments/exec-reliability/evidence/expired-request-controls-k137/report.md`. The control is `test_persistent_worker_expired_request_is_sticky` in `experiments/exec-reliability/test_worker_capture.py`. The runtime sites are `Worker.evidence` and `capture_helper` in `experiments/exec-reliability/worker_capture.py`. The runner is `run_condition.py` in the k137 evidence directory.

## Done when

- Classify each of F1 to F3 (contract unclear, real issue, visible trade-off or noise) with evidence. Re-read the source and the retained receipts. Do not inherit the k141 reading.
- For F1, decide whether the project Python 3.12 gate needs a control that rejects the `false-residual` mutant. Any new control must use real child state. Do not add a skip-free path that consumes status through `os.waitid` on 3.12.
- For F2, decide whether a deterministic schedule can reach the collected branch of `natural` without a race. If none exists, record the limit in the report path that k140 reads.
- For F3, add a frozen mutant for the receipt remedy at `capture_helper`, and decide on ECHILD and `ownership_lost`. Decide how a missing journal is reported by the audit.
- Keep every prior sample, mutant and receipt unchanged. Use fresh output directories. Retain every failure.

## Notes

No guests, privilege, installed changes or unrelated processes. Do not accept this slice as concurrent ownership, teardown overlap or aggregate acceptance. k28 and k17 stay open. The historical failures listed in the k137 report stay owned by k140.

## Decisions (running log)
