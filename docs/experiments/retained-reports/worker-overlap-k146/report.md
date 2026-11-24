> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Production worker shutdown during public teardown

The controller now advances failed inspection worker shutdown while public cleanup and disposal remain active.
It preserves the original request cutoff across command transitions.
A failed process table also retires the worker, even when helper transport completed cleanly.
Later workloads remain unreached, while public safety commands still run.

Two interleaved candidate runs pass all 949 diagnostic cases and their static checks.
Both parent runs fail the eight new overlap controls and pass the other 941 cases.
These results support independent review of the complete bounded-inspection-k28 implementation.
They do not certify guest behaviour, hard real-time scheduling or the separate storage and retained-file contracts.
Independent review and any necessary reconciliation precede k21.

The k17 node remains open through that review and retained-evidence work.
Guest acceptance remains **0/10, unavailable**.

## Production change

`controller.py` routes workload waiting, termination and exact leader reaping through `_InspectionState.drive`.
The driver registers the active command and pending worker retirement with `run_turns`.
Worker retirement has safety priority.
When a command finishes, its companion stops without waiting for worker completion.
The next command resumes the same worker state and absolute deadlines.

`capture_helper` can transfer its existing failure-retirement turn to the controller.
That turn retains bounded cleanup-only control parsing, helper identities and the original request deadline.
The controller starts worker close only after that parser finishes.
Close derives its deadlines from the earlier cutoff and attempted signals.
It cannot renew the request allowance or repeat an attempted signal.
Standalone callers retain synchronous retirement by default.

An unknown attempted process-table query makes local inspection failure sticky.
It also sets worker failure and starts close, unless request retirement already owns the worker.
Subsequent public cleanup, disposal and absence commands receive failed local inspection without another helper query.
A valid table containing live members remains a different outcome.
That outcome does not poison later inspection transport.

## Runtime ownership and waits

This inventory covers the diagnostic runtime, rather than the historical frozen copies under evidence directories.
The relevant runtime modules are `controller.py`, `worker_capture.py`, `inspection.py` and `turns.py`.
The following table distinguishes parent scheduling from operations isolated inside the worker.

| Owner or operation | Production path and authority |
| --- | --- |
| Public workload or safety command | `_call` creates a fresh session and one `_WorkloadProcess`. `_WorkloadWait` performs one non-consuming exit observation per turn. |
| Workload termination | `_Termination.advance` retains immutable TERM, KILL and inspection cutoffs. Group KILL remains mandatory even after leader exit. PID-only mode never widens authority. |
| Final group inspection | `_final_snapshot` permits one query after observed exit and the final signal, before the inspection cutoff. `_group_snapshot` checks the table before its permission probe. |
| Exact workload reap | `_LeaderReap.advance` uses exact-child WNOHANG. Its initial attempt exception is distinct from worker lifetime accounting. Later expired callbacks cannot poll. |
| Persistent worker bootstrap | `Worker.start` precedes acquisition. Its readiness turn runs alone because no public command exists yet. Failed startup closes before acquisition. |
| Persistent request | `capture_helper` dispatches through a bounded nonblocking write turn. Control and result channels have separate buffers and shared read limits. |
| Failed request | The deferred safety turn sends the existing retirement signals and consumes bounded late controls. It cannot accept a late success or resolve a helper by worker reap. |
| Worker close | `_InspectionState.advance_worker` follows request retirement with `Worker.advance_close`. Healthy final close runs after public commands. Failed close also advances beside their waits, termination and reaping. |
| Known helper | The worker owns its exact child. The controller retains published identities until a matching accepted reap receipt. Worker group signals cannot discharge that obligation. |
| Unknown creation | Missing publication remains an explicit unknown obligation. Only a valid fork-error record with clean channel completion establishes no child. |
| Destructors and residuals | `_WorkloadProcess.__del__` suppresses Popen polling. Strong owner collections retain unresolved workload, worker and helper obligations. No destructor supplies a private wait. |
| Scheduler sleep | `run_turns` selects due safety work before ordinary work, then earliest wake and stable rotation. Readiness is advisory. Sleeps remain bounded by the next wake and 5 ms. |
| Worker-local loops | `_service` receives commands in the isolated worker. `_write` handles child publication there. `inspection.capture_helper` bounds helper output and exact-child reaping locally. These loops are not controller scheduler loops. |
| Gate and fork | The isolated child gate uses bounded reads. Raw worker fork remains a synchronous OS operation. Neither mechanism establishes a hard real-time kernel guarantee. |

Public commands remain sequential by plan design.
Worker failure retirement can now coexist with each active safety command.
A helper query follows workload exit observation and precedes exact leader reap.
Its request cutoff remains inside that workload's inspection allowance.
There is no queued retry or replacement worker.

File creation, hashing and receipt writes can still block on storage.
That hazard remains k18's separate contract.
Retained evidence identity remains k21's contract.
The process scheduler does not establish either guarantee.

## Real overlap controls

The new test calls production `execute`, rather than a standalone driver seam.
Its eight cases combine table or transport failure, acquisition or workload failure, and held cleanup or disposal.
The worker is a real direct child that ignores TERM and refuses orderly exit.
Helpers retain their normal behaviour.
Transport damage occurs after the real worker reaps its helper.
The malformed result still leaves the controller's helper obligation unresolved.

Each case requires actual worker TERM, KILL and reap before the held command exits.
It also requires public cleanup, disposal and absence attempts, unreached later workloads and no further process-table request.
An independent `/bin/ps` query checks exact worker, workload and published helper identities.
An exact waitpid check requires ECHILD for the worker after runtime reap.
The audit records any remaining fake marker before removing that exact fixture marker.

Transport reserve KILL can occur during cleanup before a later held disposal starts.
The transport disposal control therefore requires overlap with teardown, rather than a delayed KILL during disposal.
Table failures and held cleanup still require KILL after the selected held command starts.
Transport controls require final owner and close deadlines no later than the original failed request cutoff.
They retain helper obligations despite independent observations of absence.

## Existing control inventory

The full runs execute all six test modules.
This map identifies the material controls for independent review.
Passing cases establish their tested conditions, rather than an exhaustive proof under every OS schedule.

| Contract | Controls in the frozen candidate sources |
| --- | --- |
| Worker before acquisition, reuse and preparation failure | `test_persistent_worker_starts_before_resources_and_reuses_for_safety`, `test_persistent_worker_startup_failure_leaves_resources_unacquired`, `test_persistent_worker_closes_when_safety_adapter_raises` in `test_controller.py`. |
| Bootstrap and shutdown bounds | `test_declared_bootstrap_real_child_bounds`, `test_startup_frame_written_at_real_cutoff_is_late` in `test_bootstrap.py`. `test_declared_shutdown_real_child_phases`, incremental shutdown and fixed-cutoff controls in `test_shutdown.py`. |
| Admission and inherited state | Native-thread, Python-thread and native/Python SIGCHLD controls in `test_inspection.py` and `test_worker_capture.py`. `test_review_fork_boundary_rechecks_prepared_state` rechecks the fork boundary. Reset controls cover handlers, masks, wakeup descriptors, hooks and evidence descriptors. |
| Creation holds and helper isolation | Delayed creation, publication loss, raw helper reset, gate token/EOF rejection and `test_review_worker_group_kill_reaches_live_raw_helper`. Real helper KILL and delayed wait return remain distinct from simulated unreapability. |
| Identity and fork errors | Request identity controls bind run, workload, worker, request and signal epoch. Created identity conflicts, late cleanup-only identity, explicit fork failure, partial channels and EOF ordering have separate controls. |
| Channel and table caps | Hostile channel and result framing controls cover fragment, duplicate, partial, overflow, stderr and late data. Turn controls check combined reads and backpressure. Table controls check row, byte, state and identity limits before probing. |
| Signal scope and final query | PID/group opposed-child outcomes, wrong scope, final-query exit gating, KILL-before-inspection, live final table, no retry and signal-permission uncertainty controls in `test_controller.py`. |
| Timing and ownership | Near-T exit, pinning, real early reap, starved callback, post-cutoff observation and fixed reap cutoff controls. Worker first-reap history spans request polling and close. |
| Shared scheduling | Safety priority, earliest wake, rotation, real pipe readiness and real worker/workload controls in `test_turns.py`. New production controls exercise the actual cleanup/disposal path. |
| Failure and public safety | Sticky inspection, worker death, reverse safety order, blocked cleanup/disposal and receipt exceptions in `test_controller.py`. Raising driver controls check owned descriptors close without private fallback. |

Real privileged ps KILL and real UID-based EPERM controls remain unavailable.
`unavailable-controls.json` names the missing prerequisites, sender identities and unreached fixture fields.
The receipt also identifies the installed setuid ps executable without claiming signal delivery.
No privilege was acquired.
Injected errno controls test uncertainty handling and do not substitute for those fixtures.
Simulated delayed observation or reap does not establish behaviour of an uninterruptible kernel task.

## Frozen measurements

The declared order was parent, candidate, parent, candidate, followed by the three mutations.
Each condition has fresh output, one writer and before/after subject hashes.
Subjects include sources, manifests, lock, Taskfile, runner tooling, comparison plan, specification, interpreter and helper executables.
The runner retains commands, JUnit outcomes, raw streams, process receipts and artifact hashes.
No measured source changed during a condition.

| Condition | Passed / failed | Result |
| --- | --- | --- |
| `parent-full-1` | 941 / 8 | New production overlap controls fail. Static checks pass. |
| `candidate-full-1` | 949 / 0 | Runtime and static checks pass. |
| `parent-full-2` | 941 / 8 | Same eight control identities fail. Static checks pass. |
| `candidate-full-2` | 949 / 0 | Same 949 identities pass. Static checks pass. |
| `skip-companion-v2` | 0 / 8 | Removing worker advancement prevents overlap. |
| `skip-failure-retirement-v2` | 4 / 4 | Table failure no longer retires the worker. Transport cases pass. |
| `synchronous-retirement-v2` | 4 / 4 | Transport retirement delays public teardown. Table cases pass. |
| `reusable-self-check` | 949 / 0 | Repository self-check, Ruff, formatting and strict Pyright pass. |
| `reusable-review-controls` | 238 / 0 | Repository review selection passes, with 711 deselected cases. |

`comparison-final-v2.json` compares exact case identities and checks all recorded artifact hashes.
It also checks sixteen overlap receipts across the two candidate runs.
Nine audit-tool controls reject changed subjects, changed artifacts, unreaped checkers, live helpers, wrong identities, renewed deadlines, duplicate signals and missing receipts.
The complete eight-receipt positive fixture passes.
Tooling Ruff, formatting and strict Pyright pass.
The final frozen static condition is `final-tooling-static-v4`.

The runtime runs use isolated locked Python 3.12.
The reusable tasks use `UV_PYTHON=3.12`.
The machine's global Python 3.15.1 is not the measured runtime condition.
Control, full and static budgets remain 60, 120 and 30 seconds respectively.
Checker timeout cleanup permits TERM for two seconds, then KILL/reap for two seconds.
These checker bounds are separate from process ownership cutoffs inside the runtime.

## Retained failures and limits

All initial fixture and tooling failures remain in this directory.
The first worker wrapper could not hold the service after its direct `os._exit`.
The next reset wrapper omitted `new_session`, which broke helper reset.
The updated `red-v3-controls` established the table-failure bug.
`red-transport-controls` then established the transport retirement bug against the table-only implementation.

The first transport candidate passed six cases and failed two overly strict disposal timing assertions.
The next candidate rejected four equality assertions because close legitimately tightened an earlier request cutoff.
The updated assertions require no renewed authority.

Initial static failures and the frozen-input preparation path error also remain retained.
The final tooling checks first exposed an unresolved `prepare` import in the independent audit.
The audit now defines its mutation expectations independently.
A subsequent line-length failure also remains retained before the passing final static condition.

Fresh `*-v2-inputs` directories preserve the updated mutant preparations.
No failed sample was overwritten or counted as passing evidence.

The historical k145 failure remains `assert -9 == 0` in the interrupted live-group case.
Its receipt records PID TERM, later PID KILL and a reaped leader with return code -9.
It has no child-readiness timestamp.
The frozen peer ignored its stopping flag while awaiting child readiness.
Current k143 peer code checks that flag, with separate held-readiness controls.
This comparison identifies a relevant changed mechanism without proving the historical failure's cause.

Earlier timing, setup, unsupported table-state and ps timeout failures remain historical failures.
Current passing cases do not retrospectively resolve those observations.
The report claims tested scheduler behaviour under the recorded conditions.
It does not claim that every runtime deadline succeeds under arbitrary OS scheduling.
Late work must still produce failed timing and retained residual ownership.

Graph generation `2026-11-15T02:19:52Z` contains changed or missing metadata for relevant scheduler paths.
Coverage checks found no recorded gaps, which does not prove completeness.
Material runtime claims use current source reads.
No graph absence claim supports this report.

## Reproduction

Use the repository Taskfile from the workspace root.
Choose fresh output directories for every measurement.

```sh
mise exec -- task diagnostic:overlap-prepare INPUTS=experiments/exec-reliability OUT=/tmp/overlap-inputs-new
mise exec -- task diagnostic:overlap-check INPUTS=/tmp/overlap-inputs-new OUT=/tmp/overlap-result-new MODE=full
UV_PYTHON=3.12 mise exec -- task diagnostic:self-check OUT=experiments/exec-reliability/evidence/self-check-new
UV_PYTHON=3.12 mise exec -- task diagnostic:review-controls OUT=experiments/exec-reliability/evidence/review-controls-new
mise exec -- task diagnostic:overlap-tools
mise exec -- task diagnostic:overlap-audit OUT=/tmp/overlap-comparison-new.json
```

`diagnostic:overlap-prepare` accepts `BASELINE` for the two runtime modules and `MUTANT` for an exact single-site replacement.
The retained preparation receipts identify the actual baseline and mutations.
The original comparison plan remains unchanged.

Prior evidence preservation excludes this new directory and interpreter, test and lint caches.
The before inventory covers 293,984 earlier evidence paths.
The after receipt matches all 293,984 paths with the same content commitment.
Its SHA-256 is `d77d3f890df56ce2fe56be22a6aa063eb7684eb9aa29836bdc9e55ba18ba99be`.

The next independent review must target **bounded-inspection-k28** before retained-evidence-k21.
Its subject includes this production change, the whole runtime trace and the inherited process-inspection contract.
No empty integration task accompanies this producer.
