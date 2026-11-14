# worker-deadline-evidence-k87

**Integrates:** complete-worker-scheduler-k86

## Goal

Triage the three evidence findings of the k86 review of the k83 worker shutdown deadline slice. Apply each real one. Keep the k83 runtime guards unchanged unless a finding proves them wrong. Keep k85's full charter unchanged.

## Context

Read the findings from the k86 commit, not from this body. Reviewed commit: c6844b2 (complete-worker-scheduler-k83). Source: `experiments/exec-reliability/worker_capture.py`, `test_worker_capture.py:16-117`, `self_check.py`. Evidence: `experiments/exec-reliability/evidence/worker-deadline-k83/`.

Findings, in order of consequence:

1. The full self-check used 54.29 s of a 60 s pytest budget. The k81 run used 44.54 s.
2. `audit.py` cannot repeat after k85 edits the live sources. The report cites a jj path comparison with no retained artifact.
3. The new control does not show each guard failing alone, and its equality case never runs inside the cleanup loop.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a trade-off or noise. Record the class in the decision log.
- For finding 1, measure the cause of the growth. Then raise the budget or shorten the suite, and state the margin in the report.
- For finding 2, state in the report that the audit is valid only for commit c6844b2. Retain the jj comparison output, or remove the claim.
- For finding 3, retain single-guard mutation RED runs for `_signal` and `_reap`. Add an equality case inside the cleanup loop, or record why it is not needed.
- Run the full diagnostic self-check and retain its evidence in a new directory. Preserve all earlier evidence bytes.
