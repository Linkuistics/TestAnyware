# group-kill-ordering-k35

**Reviews:** group-kill-ordering-k33


## Goal
Adversarially review the real controller's group KILL ordering slice before
bounded-inspection-k34 replaces its synchronous inspection path.



## Context
Read the producer, parent brief chain, diagnostic process-inspection spec and
the report's k33 section. The narrow delivered contract is no process-table
query during group TERM grace, group KILL at fixed T before inspection even
after leader exit, fixed group K=T+kill_wait, pinned leader identity, narrow
conditional PID-only signalling and sticky earlier TERM permission uncertainty.
This is deliberately incomplete relative to k30's worker/scheduler contract.

## Done when
- Inspect the actual signal, exit observation, query and reap paths, including
  normal exit, PID-only interruption, graceful group exit, stubborn children,
  denied signals and observation overrun. Contest whether any synchronous query
  still precedes group KILL or a later table repairs an earlier denied TERM.
- Review real-process controls, early-reap/scope/deadline mutations and the
  retained per-item evidence. Read k33-red-01, both full runs, both exact input
  snapshots and both audits. Distinguish the four meaningful red failures from
  the migrated mutation seam, and the macOS zombie-only KILL alternative from
  permission certainty. Check report claims against receipts and hashes.
- Audit the independent later PID observations, exact scratch cleanup and failed
  first audit. Do not infer containment of unreported helpers or escaped children
  from later absence, or a hard inspection bound from passing timings.
- Identify correctness, regression, evidence or handoff findings with precise
  source/evidence references, or explain a justified pass. Keep full worker,
  protocol, concurrency, destructor/wait and complete inventory obligations on
  k30/k34; this review cannot certify those undelivered properties.
- If actionable findings need integration, insert it immediately before k34
  using the producer's bare stem and the review handle. Do not cut an empty
  integration.

## Notes
The delivered condition is k33-final-02 (75 tests plus lint/format/strict types).
The independent final audit is k33-audit-02. Post-KILL subprocess capture,
helper kill/wait and blocking leader reap remain known limitations. Historical
guest acceptance remains 0/10, unavailable.


## Decisions (running log)
**Method.** Inspection only. No test, build, lint or format command ran. I read
`controller.py` `_terminate` (237-333) and `_call` (336-426), the k33 diff in
commit 0c00d85, the report section, the spec and the retained receipts.

**Verified, no finding.** The report matches the receipts on these claims.
Controller sha256 `1fc6e51b…` equals the committed and worktree file. The
final-02 verdict is ok with lint, format, types and 75 tests clean.

**Red run.** The red run has 62 tests with 5 failures. Four are behavioural: KILL
missing, query before KILL, permission uncertainty lost and earlier EPERM
repaired. One is the migrated mutation seam. Its string
`"term_deadline + plan.kill_wait"` has count 0 in the red source.

**Red inputs.** Both input snapshots hash to their manifests. The red controller
equals the parent commit file.

**Ordering and audit.** Group KILL fires 0.66 ms and 1.84 ms after T in the two
final-02 ordering receipts. Both precede the first query. The hang case shows
real macOS `zombie_only_eperm` on KILL. That is the zombie-only alternative and
not permission certainty.

**Audit.** Audit-02 totals match the report: 1,345 artifacts,
1,393 inputs, 257 PIDs absent, 77 resources, 2 markers and 212 directories.
The audit-01 failure names retained PID 23889 and stopped before cleanup.

**Finding 1 (test coverage, medium).** The PID-only kill-deadline cap has no
mutation control after k33. In the parent test the seam mutated
`min(time.monotonic(), term_deadline)` inside `_terminate`. k33 split the
formula, so the retained seam at `test_controller.py:867-871` now mutates
`term_deadline + plan.kill_wait`, which is the group branch only. The PID-only
`min(time.monotonic(), term_deadline) + plan.kill_wait` at `controller.py:310`
can be removed and every retained test still passes. The spec K=min(G,T)+A for
PID-only calls is therefore unguarded.

**Finding 2 (evidence claim, low).** The report says audit-02 removed "212 exact
fixture directories" (`docs/experiments/exec-reliability-diagnostic-foundation.md`
line 682). Only 107 of those 212 directories appear in retained argv paths. The
other 105 came from `audit.py:136-144`, which adds every `test_*` directory in
each session. The sessions are pytest-81, pytest-82 and pytest-83, matching the
three conditions, so the deletion probably stayed inside k33 runs. The receipts
cannot show that, and the word "exact" overstates it.

**Finding 3 (contract, low, not reproduced).** Real TERM `EPERM` on a group that
is already zombie-only gives `permission_unknown` for ever
(`controller.py:283-286`). Only the injected EPERM controls exercise TERM. A
leader that exits between the deadline and the TERM call would then fail
`termination_ok` falsely. The direction is fail-safe, so this is a trade-off to
accept or to state in the contract.

**Not findings.** Repeated synchronous `ps` after KILL, the blocking
`subprocess.run` timeout path and the blocking leader reap are known k34
limitations. The review does not certify them, nor worker, protocol or
concurrency properties. Later PID absence proves no containment of escaped
children and the report says so. Passing timings prove no hard inspection bound.

**Outcome.** Actionable. Integration leaf `group-kill-ordering-k36` is inserted
before `bounded-inspection-k34`.
