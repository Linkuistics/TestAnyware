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

## Review outcome

Six actionable findings are recorded in
`docs/experiments/exec-reliability-macos-completion-review-k190.md`, against
source commit `4b23ff10258b0d5e387482917c86b99547b5e006` and evidence commit
`c414438b40a8df487347cebb81abc2b0514ea0b5`. The retained 22 receipts support
the observed clean run; missing-sample acceptance, JSON null acceptance,
unobserved inner pipe EOF, hidden failure exits, omitted tooling hashes and
unstated statistical assumptions prevent relying on the reusable verdict
path and its unqualified bounds.

This was an inspection-only review. No production/test code changed and no
test, build, lint, format or live diagnostic command ran. Findings describe
source-derived failure cases, not induced failures in the retained run.

## Decisions (running log)

The six findings affect explicit complete-receipt, evidence-freeze or
diagnostic-inference acceptance; they meet the material-risk bar. Cut one
`integrate-review-impl` sibling using the producer stem
`macos-completion-live`, immediately before the next live sibling k4. It owns
triage, fixes and verification and consumes this review commit. Keep the
original frozen-source commit separate and the historical evidence intact.
Other live root siblings remain, so retirement closes no parent node.
