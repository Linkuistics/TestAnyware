# complete-worker-scheduler-k145

**Integrates:** complete-worker-scheduler-k144

## Goal

Triage findings F1 to F4 of complete-worker-scheduler-k144 against the k140 caller-scheduling boundary. Apply the real part narrowly. Leaf k143 keeps the aggregate charter, production owner scheduling and overlapping teardown.

## Context

Read the k144 Decisions log first. The runtime sites are `capture_helper`, `Worker.close` and `Worker.begin_close` in `experiments/exec-reliability/worker_capture.py`. The retirement Turn is scheduled at line 1389, inside the `finally` that starts at line 1327. The tests are in `experiments/exec-reliability/test_turns.py`. The runner is `experiments/exec-reliability/evidence/caller-scheduling-k140/run_condition.py`. Re-read the source and the retained receipts. Do not inherit the k144 reading.

## Done when

- F1: decide whether a supplied driver that raises for the retirement Turn or the close Turn must still close the worker channels. If so, add a deterministic control with real descriptors. Reject a mutant that skips the channel close. Keep the exception visible to the caller.
- F2: decide whether a driver that returns without running its Turn needs a runtime check or only a documented contract. Record the decision where k140 reads it.
- F3: correct the report.md sentence at line 28 about the query-bypass mutant. Name the two secondary `assert (1 == 2)` rejections at `test_turns.py:447`. Do not edit the retained mutant result directories.
- F4: record the run_condition.py change (ebb966a5 to 649f85d1) in an addendum to observed-deviations.json or a new file. Decide whether `caller_audits` must validate journal contents. Preserve every retained sample, mutant and receipt byte-for-byte. Use fresh output directories.
- Classify each finding (contract unclear, real issue, visible trade-off or noise) with evidence before acting.

## Notes

Cut by complete-worker-scheduler-k144 using `leaf-insert`, because the k143 impl leaf was live.
