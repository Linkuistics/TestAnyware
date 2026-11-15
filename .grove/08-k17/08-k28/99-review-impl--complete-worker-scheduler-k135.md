# complete-worker-scheduler-k135

**Reviews:** complete-worker-scheduler-k132

## Goal

Adversarially review readiness wake dispatch and the request/retirement interest registrations before the aggregate scheduler resumes.

## Context

Read the producer commit and `experiments/exec-reliability/evidence/readiness-wakes-k132/report.md`, then inspect `turns.py`, the query and retirement callbacks in `worker_capture.py`, `test_turns.py` and the frozen comparison auditor. Read k134 for the intact aggregate contract and unresolved historical failures. Production owners still run sequentially; concurrency, teardown overlap and the final k28 review remain open. The new registration only lets the common driver wake idle bounded callbacks.

## Done when

- Challenge due-safety priority when readiness arrives before, at or after a safety deadline, earliest-timer waiting, initial dispatch, rotation, multiple interests, spurious readiness and terminal removal. Consider starvation and readiness errors without introducing private owner waits.
- Inspect interest transitions from command writer to stream readers and cleanup-reader closure, immutable cutoff checks, per-turn I/O caps and sticky failure. Distinguish registered transport interests from actual concurrent production ownership.
- Verify exact-clock/real-pipe controls, real production registrations and six rejecting mutants. Inspect per-item interleaved parent/candidate results, tool/subject hashes, fixture absence audits and prior-evidence preservation. Retain every failure; clean reruns do not explain old failures.
- Record actionable findings with source/evidence anchors. If action is needed, insert an integration leaf before k134. Do not close k28/k17 or accept the aggregate on the strength of this seam.

## Notes

The producer report records final results and any retained comparison rejection. Privileged ps/UID controls and ten-clone guest acceptance remain unavailable. No privilege acquisition or guest changes are authorized by this seam.

Inspect the original three parent workload timing failures and recurring final-parent expired-request failure directly; later candidate passes do not explain them. Verify the explicit pending-list annotation and the auditor import repair against preserved instrument copies and both current frozen static checks. Distinguish integrity verification, current static success, candidate runtime success and the deliberately rejected comparison gates. The producer report supplies the exact source baseline and retained condition paths.
