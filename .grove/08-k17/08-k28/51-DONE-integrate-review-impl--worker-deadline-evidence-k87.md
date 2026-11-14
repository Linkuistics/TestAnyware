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

## Decisions (running log)

- Finding 1 is a real evidence issue: the retained JUnit sums grew from 44.207 s to 53.705 s; new deadline cases explain 2.421 s and shared cases explain 7.077 s. Measure frozen k81/k83 suites with one bounded runner to distinguish persistent change from run variation before choosing margin.
- Finding 2 is a real reproducibility issue. Preserve k83 bytes; the new report will qualify the old auditor to c6844b2 and supersede its unsupported path-comparison claim with retained jj output plus a new prior-file inventory.
- Finding 3 is a real control gap. Retain independent single-guard mutations and add cleanup-loop equality triggered on its KILL entry. Runtime guards remain unchanged; k85 retains its entire charter.
- Sequential frozen replays expose broader variability: k81 original copy 46.89 s with a relocated Taskfile failure; k83 original copy 75.02 s with that failure and existing process timing controls failing; relocated k81 54.89 s with the real-ps failure control failing. Preserve these failures by identity rather than counting them as green. Raise the normal full-suite budget to 120 s (not the preliminary 90 s) to leave at least 44.98 s against the slowest observed replay; local test deadlines and checker cleanup bounds remain fixed. Timing replay runner remains frozen at its declared 90 s.
- Single-signal mutation produced exactly 8/10 expected late-signal failures; single-reap mutation exactly 10/10 late-wait failures. All 10 green controls passed, including cleanup-loop equality in PID and group scope. Frozen relocated k83 passed 432/432 in 50.72 s. Full-1 lint/format/types passed; pytest 433/434 in 47.31 s, with existing `test_late_created_identity_rejects_invalid_fields[token]` failing its reaped expectation. Preserve full-1 and repeat the unchanged full check as full-2; do not silently promote the failed run.

- 2026-11-15: Completed the evidence repair. Unchanged full-2 passed lint/format/types and all 434 tests in 47.246761708s, leaving 72.753238292s against the declared 120s pytest allowance. full-1 stays failed (433/434, token late-identity reaped-child assertion); all exploratory replay failures remain named in the report and audit. audit-1 passed with 90,462 prior entries and no differences, per-case outcomes/times, 50 exact-child control audits, isolated guard mutations, frozen sources and artifacts, and actual jj comparison receipts. audit-check-1 retains a Ruff E501 failure; formatted auditor then passed in audit-check-2 under its 60s outer bound in 7.622s. Report: experiments/exec-reliability/evidence/worker-deadline-k87/report.md. Runtime worker_capture.py and the entire k85 charter are unchanged. This repairs all three k86 findings and makes no aggregate k28 completion claim.
