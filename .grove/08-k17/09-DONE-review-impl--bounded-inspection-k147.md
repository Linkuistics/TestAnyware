# bounded-inspection-k147

**Reviews:** bounded-inspection-k28

## Goal

Review the complete bounded-inspection production contract before retained-evidence-k21 consumes its observations and residual inputs.

## Context

Read k28's brief, k17's promoted acceptance requirements and `docs/specs/diagnostic-process-inspection.md` in full.
Review the commit that retires complete-worker-scheduler-k146, including its current runtime and frozen source condition.
Use `experiments/exec-reliability/evidence/worker-overlap-k146/report.md` for the source, control and retained-failure inventory.
The report links both interleaved full comparisons, mutations, independent overlap audits, reusable checks and unavailable-control receipts.
Read earlier reports where the current contract relies on their controls or reconciliation.

Trace actual `execute` and all runtime ownership through `controller.py`, `worker_capture.py`, `inspection.py`, `turns.py`, `peer.py` and their callers.
Include preparation, foreground workloads, reverse resource cleanup, disposal, public absence commands and final close.
Review the existing full control inventory as well as the eight new production overlap cases.
The optional standalone driver interface alone cannot establish production scheduling.

## Done when

- Trace helper intent, fork admission/reset, gated identity publication, exec, output drains, late control parsing and exact reap.
  Distinguish explicit fork failure, unknown creation and independently retained helper obligations.
- Trace every parent wait, readiness callback, signal entry point, destructor and shutdown hook.
  Check workload and worker authority separately, including early reap, stale callbacks and cutoff closure.
  Check fixed T/K/R, mandatory group KILL, conditional PID KILL and the sole final query.
- Challenge production failure retirement while cleanup or disposal remains open.
  Check safety priority, channel caps, original worker deadlines, already-retiring close and request retirement across command transitions.
  Check preparation exits, healthy close, caller exceptions and residual ownership without private blocking cleanup.
- Independently check source identities, frozen subjects, artifact hashes, case identities, mutation rejection and exact fixture cleanup.
  Check each distinct worker, helper, workload leader and observed-member obligation.
  A worker reap cannot discharge another owner, and a damaged receipt stays failed.
- Reconcile historical timing failures and k145's unresolved interrupted live-group failure without retrospective correction claims.
  Check the actual setuid-ps KILL and genuine UID-denial unavailable receipts against the stated prerequisites.
  Missing real controls prevent a full guarantee, and guest acceptance remains 0/10 unavailable.
- Record findings against the complete k28 contract before k21 proceeds.
  Create reconciliation work only for actionable findings, with no empty integration.
  If runtime defeats the contract, externalise the precise counterexample to design before its consumer.
  Do not extend ceilings, add threads or workers, acquire privilege or widen signal scope.

## Consumer boundary

k21 owns stream and result identity, surviving writers and retained-file late changes.
k18 owns stalled evidence storage and teardown evidence descriptors.
Neither boundary permits publication acknowledgements to gate helper exec, workload signals, reap or disposal.
k17 remains open until its complete acceptance requirements hold after review reconciliation and retained-evidence work.

## Decisions (running log)

The inspection of the complete runtime and retained controls establishes no new actionable defect against the agreed bounded-inspection contract. No integration leaf is warranted. This permits k21 to consume the reviewed observations with the limitations below; it does not establish the unavailable real permission controls, strict success under every OS schedule, retained-file integrity, storage isolation or guest acceptance.

## Review result

Reviewed producer commit `33c7cee4` (`complete-worker-scheduler-k146`) and the current diagnostic runtime, including unchanged modules and the `peer.main` and `preflight.main` callers of `execute`. The review used source and committed verification records only. No tests, builds, linters or formatters were run, and no implementation or test source was changed.

The graph project was `Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`, generation `2026-11-15T02:19:52Z`. Exact runtime queries and call traces provided discovery, but stale line ranges and missing current scheduler symbols required direct source reads. Coverage checks for the runtime, callers, controls and cited evidence reported no recorded issue, with changed or untracked metadata on material paths. This is not completeness evidence. The conclusions below rely on current source and the retained artifacts.

### Ownership and scheduling

`controller.py` registers workload wait, termination and exact leader reap through `_InspectionState.drive`. Its companion advances failed request retirement or worker close with safety priority, while the active public command remains scheduled. A finished command yields to the next command without waiting for worker shutdown. Retirement state survives those transitions. Reverse cleanup, disposal and public absence commands remain attempted after a failed acquisition or workload; subsequent workloads remain unreached.

`worker_capture.py` transfers the existing failed-request retirement turn to that companion. Cleanup-only parsing can retain a late helper identity but cannot accept a late success. Close follows that parser and uses the original cutoff and attempted signals. A clean transport with an unknown table also makes inspection sticky and starts worker close. A valid table with live members remains a distinct failure and does not poison transport. The healthy final close and preparation-failure close retain their declared bounds.

Workload TERM, fixed group KILL at T and the sole post-exit final query remain under controller authority. PID-only termination does not acquire group authority. Inspection closes at K-R, and exact leader reaping preserves the residual on failure or lateness. Popen destruction supplies no implicit poll or reap. Worker reaping has its own lifetime-first-attempt rule; it cannot resolve another owner. Existing raising-driver controls preserve exception visibility and close descriptors without a private fallback scheduler.

The helper path rechecks single-threaded/native/Python SIGCHLD admission at the fork boundary, resets inherited child state, publishes intent and pinned creation before releasing the private exec gate, and uses `os._exit`. Publication needs no controller acknowledgement. Explicit fork failure requires a timely validated record and both real EOFs before the original deadline with no conflict; an empty open channel remains unknown. Accepted matching helper reap receipts alone discharge published helper obligations. Lost or damaged terminal data retains those obligations even after worker reap or an independent absence observation.

The scheduler and protocol checks retain safety-first due ordering, bounded readiness waits, at most 64 KiB combined channel reads per turn, separate bounded framing buffers, request/epoch identity and one terminal result. The inspected full control inventory includes admission/reset, startup/close, actual fork/EOF states, caps and backpressure, signal scope and permission uncertainty, final-query ordering, early-reap rejection, stale callbacks, cutoff closure and residual receipts. Real pipes/processes and injected-clock or errno controls remain identified separately; none proves an uninterruptible kernel operation is bounded.

### Independent retained-evidence checks

Read the [production report](../../experiments/exec-reliability/evidence/worker-overlap-k146/report.md), comparison declaration, runner, preparation tool, auditor and its nine controls. Independent read-only hash/XML inspection, without invoking those test or audit commands, confirmed:

- Both full candidate runs contain the same 949 identities and no failures. Both parent runs contain those identities with the same eight overlap failures and 941 passes. The frozen candidate Python sources, manifest and lock match the current diagnostic inputs.
- The three exact single-site mutants retain the declared eight-case selection: skip-companion fails all eight; skip-failure-retirement fails the four table cases; synchronous-retirement fails the four transport cases. The other four cases pass in each of the latter conditions.
- All 8,916 recorded artifacts across those seven conditions match their recorded hashes. All 203 measured subject entries agree before and after and with their declarations. Current tooling differs from some historical measured subjects: `prepare.py` in the four full conditions, and `run_condition.py` plus root `Taskfile.yml` in all seven. Those are later instrument edits documented by the retained tooling history; they do not establish within-run source drift or a runtime repair. Frozen runtime bytes match.
- Sixteen candidate overlap receipts bind the exact table/transport, acquisition/workload and held-cleanup/disposal cases to their result and lifecycle receipts. The independently reconstructed identity sets contain 16 workers, 72 workload leaders and 24 published helpers. Their exact-PID absence receipts report return code 1 and empty output/error. Worker receipts retain one TERM, one KILL and runtime reap with return code -9, before the held command exits. Transport retirement never renews the failed request cutoff and retains its helper obligation. A transport KILL may precede held disposal because cleanup already overlaps retirement; the control does not pretend it happened during disposal.
- The reusable self-check has the same 949 identities and zero failures; the review selection has 238 passing identities drawn from that full set. Their 3,053 artifacts match, measured subjects agree before/after, and only the later root Taskfile differs today. Final-tooling-static-v4 retains successful checks with seven matching artifacts and unchanged current subjects. These are recorded results, not executions by this reviewer.
- Independently recomputing the earlier-evidence commitment gives 293,984 paths and SHA-256 `d77d3f890df56ce2fe56be22a6aa063eb7684eb9aa29836bdc9e55ba18ba99be`, matching both preservation receipts. The same exclusions and symlink treatment apply.

### Limits carried to the consumer

The historical k121 full sample remains 855 passes and a failed WNOHANG timing control, 20.291 microseconds beyond its recorded cutoff. k139's first full sample likewise retains a 347-microsecond late observation; its later 921-case pass does not repair that sample. k132's passing candidate suites coexist with its rejected aggregate comparison, unresolved parent timing cases and expired-request residual-remedy failure. Current deadline checks can reject late work; later green runs do not prove those failures impossible or establish their causes.

The k145 interrupted live-group failure remains `assert -9 == 0`. Its receipt has PID TERM, later PID KILL and leader reap with -9, but no child-readiness timestamp. The old peer readiness loop ignored its stopping flag; the current peer checks it and has separate held-readiness controls. That source difference is relevant but cannot establish the cause of the old receipt. Earlier setup, unsupported table-state, missing-publication and ps-timeout observations also remain retained adverse evidence. None is retrospectively relabelled as repaired by k146.

The unavailable-controls receipt identifies sender real/effective UID 501, the installed root-owned setuid `/bin/ps`, and null fixture/signal/reap fields for both missing real controls. An actual owned unreaped ps with a verified live signalling window is missing. A separately authorised different-UID fixture owner and its cleanup authority are missing for real EPERM plus missing-table uncertainty. Installed executable metadata and injected errors establish neither control. No privilege acquisition or new fixture execution was attempted in this review. Missing controls prevent a full guarantee.

k21 may proceed with immutable observations and independent worker/helper/workload/member residuals. It still owns surviving writers and late retained-file changes. k18 owns blocked storage and evidence descriptors. k17 remains open through retained-evidence work and its complete acceptance assessment, including the unavailable controls and unresolved historical timing evidence. Guest acceptance remains **0/10, unavailable**.
