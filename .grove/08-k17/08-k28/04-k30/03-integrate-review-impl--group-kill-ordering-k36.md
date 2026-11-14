# group-kill-ordering-k36

**Integrates:** group-kill-ordering-k35


## Goal
Triage the k35 findings on the k33 group KILL ordering slice. Fix the real ones
and accept the trade-off visibly.

## Context
Read the findings in the k35 review commit, under its `## Decisions (running
log)`. Grade each against the source and the retained evidence. Do not trust a
restatement here. The delivered condition is k33-final-02 with audit k33-audit-02.
The k34 worker, scheduler and complete-inventory work stays with k34.

## Done when
- Finding 1: a retained mutation control fails when the PID-only cap
  `min(time.monotonic(), term_deadline)` in `controller.py` `_terminate` is
  removed. The control uses a real PID-only call with a late exit observation.
  Show the control passing unmutated and failing mutated.
- Finding 2: the report sentence "212 exact fixture directories" matches the
  receipts. Either state that 107 came from retained argv paths and 105 from
  session-wide `test_*` enumeration of pytest-81, pytest-82 and pytest-83, or
  restrict the audit deletion. Rerun an audit only if the code changes.
- Finding 3: classify the sticky TERM EPERM on a real zombie-only group as a
  contract stated unclearly or a trade-off. Record it in the report.
- Every finding carries a verdict (real, trade-off, unclear contract or noise).

## Notes
k33 is already merged into the tree. Keep the PID-only change in tests unless
Finding 1 exposes a production defect. The post-KILL capture, the helper
kill/wait and the blocking leader reap stay known limitations for k34.
