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
