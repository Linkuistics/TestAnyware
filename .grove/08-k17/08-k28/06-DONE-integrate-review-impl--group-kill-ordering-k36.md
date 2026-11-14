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

## Decisions (running log)

- Finding 1 is a real test-coverage issue: the existing mutation now exercises
  only the group deadline. The PID-only production cap remains correct. Add a
  real PID-only interruption with an exit observation delayed beyond TERM grace,
  retain its cap verdict, and demonstrate rejection of the removed-cap mutation.
- Finding 2 is an unclear audit-scope claim. Correct the report to distinguish
  107 retained-argv paths from 105 session-enumerated fixture directories;
  preserve the original auditor and receipts without rerunning that audit.
- Finding 3 is a visible fail-safe trade-off: TERM EPERM remains sticky because
  a later zombie-only table cannot explain the earlier denial. Document possible
  false failure for an already zombie-only group; no inspection may delay KILL.
  Genuine permission evidence and the full worker/scheduler remain k34's scope.
- The frozen normal run passes all 63 review controls; the identical removed-cap
  tree fails only the new PID deadline assertion (62 others pass). The first
  full self-check passes lint/format/types and 75 tests, but an existing resource
  continuation test fails when its 0.2-second cleanup/disposal calls exceed their
  deadlines and their later `ps` queries time out. Preserve this condition and
  rerun unchanged inputs; do not weaken its budget or fold k34's scheduling work
  into this test-coverage repair.
- The unchanged-input k36-final-02 passes all 76 tests plus Ruff lint/format and
  strict Pyright. k36-audit-01 verifies both frozen trees, the sole removed-cap
  mutation and all four conditions' 1,742 indexed artifacts; 1,782 input hashes
  remain equal. All 332 recorded PIDs are absent at the later observation. It
  checks 100 resources, removes three owned markers and 246 validated fixture
  directories (138 retained argv, 108 session enumeration), preserving preflight
  scratch bytes. The report records raw evidence links and final digests.
- All three findings are settled within this narrow integration. Production
  controller bytes and original k33 evidence remain unchanged. The parent stays
  open because bounded-inspection-k34 still owns the complete worker/scheduler
  and its control inventory. No ADR changes or new producer review are needed
  for this regression control and evidence-scope correction.
