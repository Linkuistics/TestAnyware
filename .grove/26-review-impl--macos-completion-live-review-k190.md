# macos-completion-live-review-k190

**Reviews:** macos-completion-live-k186

## Goal
Adversarially review the live macOS completion measurement and its claims
before leaf k4 builds on the k3 report.

## Context
- Read the producer commits for `macos-completion-live-k186`, its decision
  log, this grove's root brief and the "Live clone run (k186)" section of
  `docs/experiments/exec-reliability-macos-completion.md`.
- The plan is `experiments/exec-reliability/completion_k186.py`. The
  concurrent wrapper is `concurrent_k186.py`, and the receipt checker is
  `completion_k186_check.py`, in the same directory. The outer runner is
  `completion_k186_run.sh`. The Taskfile tasks are
  `diagnostic:macos-completion-live`, `-live-check` and `-live-tools`.
- Raw evidence is in
  `experiments/exec-reliability/evidence/k186-completion-macos-01` and the
  `-outer` directory beside it.

## Done when
- Challenge whether the checker or the wrapper can report a pass without a
  prompt, complete guest receipt. Cover a truncated pipe read, a non-JSON
  receipt and a request that the wrapper killed.
- Challenge whether the four concurrent requests overlapped in the guest,
  and whether the claimed worker demand of at most about 8 holds.
- Challenge the owned-resource audit and the identity claims against the
  raw evidence. Include the dirty diff and the tart `Accessed` change.
- Challenge the cause-table changes and the statistical bounds. Check that
  the report claims no repair and no excluded cause.
- Record each finding with its evidence. Cut an integration leaf only for
  findings worth acting on.

## Notes
The guest pool read missed the agent process (`pgrep -x` found none). The
producer recorded that miss as a limit and did not repeat the read.
