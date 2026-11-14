# complete-worker-scheduler-k89

**Reviews:** complete-worker-scheduler-k85

## Goal

Independently challenge the committed workload termination cutoff admission slice before complete-worker-scheduler-k88 continues the aggregate. Re-derive findings from the producer commit and current source, with the parent contract and docs/specs/diagnostic-process-inspection.md as requirements.

## Context

The bounded artifact is controller._terminate, its new controls, Taskfile runner, retained termination-cutoff-k85 evidence/auditor/report and decomposition routing. Check the original immutable bound before first OS operation, strict equality, observer entry and returned-time checks, PID earlier cap, fixed group grace/KILL and explicit unsuccessful skipped-signal receipts. Trace actual OS entry points and callers where material; examine interactions with timely/late exit and failure paths. The guard samples do not promise atomic protection across process preemption.

Reconcile source/checker/input identities and per-case mutation evidence, failed RED attempts, full-1's retained failures and full-2's frozen-checker-only metadata lookup adjustment. Evaluate what the recorded real direct-child signals, exact owned cleanup and later absence establish. Challenge claims against the evidence rather than accepting the passing totals. Read experiments/exec-reliability/evidence/termination-cutoff-k85/report.md and audit-1/verdict.json as evidence, not review conclusions.

The full persistent pre-resource worker/common scheduler charter remains live in complete-worker-scheduler-k88. This review does not certify that aggregate, replace its mandatory final bounded-inspection-k28 review, or close k28/k17. The intermittent permission/empty-PID full-1 fixture mechanisms remain unresolved; a later pass is no repair claim. All prior evidence is immutable. No privileged ps/UID controls or ten-clone acceptance are available from this slice.

## Done when

- Record a bounded independent review of the producer diff and materially affected runtime/test/evidence paths, with precise findings, severity, source anchors and coverage/verification limits.
- Preserve failed attempts and verify any new conditions against frozen subjects; do not rewrite prior evidence.
- Cut an adjacent integrate-review-impl only if actionable findings earn it, naming this review handle without copying findings into its charter. Leave the aggregate continuation and final aggregate review obligations intact.

## Decisions (running log)

- Flat review sibling is sequenced immediately before k88 so its result can be reconciled before aggregate runtime changes. Current launch supplied no GROVE_PROVIDER_EVIDENCE; Creator is omitted per the retirement contract.

- Method: inspection only. No test, build or lint command ran. Subject: commit f5a081e (change lkolxorr). Coverage limit: no code graph was consulted. Reads were direct file reads and cmp/sha comparisons.

- Source, verified without finding: the original bound is fixed at entry, before any target operation (`controller.py` `_terminate`, `started`, `term_deadline`, `kill_deadline`, `inspection_deadline`). Observer entry rejects at equality (`>= cutoff`). Observer return is checked against the same cutoff. A skipped signal records `deadline_exhausted`. `termination_ok` accepts only `sent` and `zombie_only_eperm`, so a skipped signal always fails the call. The PID-only earlier cap is rebound after the grace loop and the `send` closure reads the rebound value. The group scope keeps T+A.

- Evidence, verified without finding: `controller.py` and `test_controller.py` in green-v2, green-v3 and the live tree are byte-identical. All 11 frozen inputs match the live tree by cmp. The green-v3 `self_check.py` differs by exactly one line, and the Taskfile digest matches the live file. Each of the six mutants differs from green-v2 by exactly one guard line. The six retained full-1 failures recur in earlier retained runs (k23, k33, k83, k87). None shows a `deadline_exhausted` outcome. The mechanisms stay unresolved, as the report states.

- Finding 1 (medium, real, runtime): signal admission closes at K, not at the inspection cutoff K minus R. The spec allocates R "for closing authority and attempting reap" and says "at cutoff, close the target epoch, disable every target signal/probe entry point". `send` compares against `kill_deadline` (`controller.py`, `time.monotonic() >= kill_deadline`). Observer entry uses `inspection_deadline`. A KILL can therefore reach the target inside [K minus R, K) after exit observation is already closed. Such a call fails anyway, so no false success results. The signal still breaks the closed-authority rule. No control covers that window. Resolution needs a reading of "cutoff" first: the leaf must decide whether K is a legal bound for signals.

- Finding 2 (low, evidence gap): the six mutants do not touch the early bound itself, the `leader_exit is not None` short-circuit or the `observer_ok = False` assignments. The early bound has only the original-runtime RED runs (red-3, red-4) and no single-guard mutant. The report claim "each guard fails alone" therefore covers the three comparison guards only.

- Finding 3 (low, receipt precision): `send` samples `event["monotonic"]` before the guard samples the clock. A `deadline_exhausted` receipt can show a time earlier than K although the guard saw K or later. Receipts cannot show the admission decision time.

- Finding 4 (low, robustness): `run_condition.py` runs the full suite only when the mode equals `full` exactly. Any other mode, such as a typo, runs the controls and reports success. The Taskfile recipe passes the mode through unchanged.

- Limit, routed to k88: `_reap_leader` makes no `waitpid` attempt when the reap deadline has already passed. The spec says to attempt a reap and poll only within K. A delayed `_terminate` return can therefore record a residual leader for a child that already exited. The k85 slice does not change `_reap_leader`, and the brief assigns reaping to k88. Record it in the k88 charter review, not in the integrate leaf.

- Limit, accepted: the real direct-child control simulates a delayed observer with a sleep. It does not model a stalled kernel call. The report states this. The guards sample time and give no atomic protection across preemption, as the task file says.

- Limit, accepted: the `_group_snapshot` monkeypatch in the observer-return control uses a two-argument fake. It does not exercise the real `inspection.snapshot` signature. The assertion on the deadline value is still meaningful.

- Split: Findings 1 to 4 earn one integrate leaf, complete-worker-scheduler k90 (`termination-signal-cutoff`), cut ahead of k88. The reap limit stays with k88. No ancestor closes. The final aggregate review obligation is unchanged.
