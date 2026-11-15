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

## Decisions

- F1 — real issue. Verified the review commit d1811ef7 and current source: a safety driver exception bypasses the remaining finalization. Eight test-first cases (RuntimeError/OSError across standalone retirement, persistent retirement, persistent close and direct close) all reject the current runtime on real pipe descriptor leakage. Repair with unconditional nonwaiting finalization; propagate the original exception and keep the original cutoff and retained ownership. No private fallback driver or new process authority.
- F2 — unclear contract, resolved as a documented synchronous driver precondition. A supplied driver must advance each Turn through completion or raise, including safety Turns. Returning early is invalid composition, not an instruction to transfer scheduling back to the module. Document at both runtime entry points and in the k140 report; no runtime fallback or completion-probing extra Turn.
- F3 — real evidence precision issue. The query-bypass result includes two secondary default-driver `assert (1 == 2)` failures at frozen test_turns.py:447. Correct only the explanatory report; preserve all result artifacts.
- F4 — real provenance omission. Keep the k140 runner and retained conditions unchanged; add an addendum with exact hashes and condition attribution. New verification will use a separately frozen k145 runner. Its caller accounting must validate declaration/completion contents, while process absence stays a separate exact fixture audit.
- Verification route: executable real-descriptor controls and a one-site skip-close mutant directly challenge this narrow repair. No duplicate reviewer of k144 is dispatched. The existing k143 aggregate path and final Reviews: bounded-inspection-k28 retain adversarial review of production scheduling/overlapping teardown; this leaf does not discharge that charter or 0/10 unavailable guest acceptance.
- Initial frozen candidate-full passed all 939 runtime cases, but failed the overall gate on one contradictory audit assertion, missing journal-list typing and five overlong string lines. Retain it as a failed condition. Correct these verification/string-layout defects and freeze candidate-v2 and both closure mutants; do not edit or replace retained results.
- Final narrow verification: all eight cleanup cases pass in both full conditions. The standalone skip-close mutant fails two descriptor assertions; the worker skip-close mutant fails six. Final audit-tool controls (12), Ruff, format and strict Pyright pass. All five conditions retain stable subjects, verified artifact digests, reaped checkers and resolved fixture accounting. See evidence/caller-cleanup-k145/report.md and comparison.json under experiments/exec-reliability.
- Final candidate-v2-full remains a failed gate: 938 pass, one failure at test_controller.py:1708, test_review_live_group_is_failure_even_with_expected_leader_exit[True], assert -9 == 0. No cause or repair is inferred from the earlier 939-pass runtime sample. Preserve both; k143 must investigate this failure with its existing aggregate obligations. Both full runs have 931 common historical k140 cases plus eight additions; the final run changes one common outcome. Narrow F1-F4 integration is complete; aggregate acceptance is not.
- Preservation passes: 285,660 earlier evidence paths retain their original commitment 747c775de1e6d5db0073b4e08816a028acd26d80f1d42ea23172eb448c3baf36 except the permitted k140 report correction. prior-after.json records the before/after report digests. No ADR premise changed. k143 remains live, so no ancestor closes. Whole-handle search finds no live review of this leaf requiring a Creator receipt.
