> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Startup frame control integration — k139

F1 from review commit `2bcff0cc725956a67088244419fb2f5bd21087c2` is a real test sensitivity issue. The existing valid-fragment/expiry controls did not isolate malformed-frame failure classification or postparse lateness. The runtime is unchanged. New real-child controls and exact frozen mutations now distinguish those branches.

## Final measurements

| Condition | Result |
| --- | --- |
| candidate-controls-5, candidate-controls-6 | 14/14 twice, identical per-case results |
| candidate-full-1 | 920 passed, one existing shutdown deadline failure |
| candidate-full-2 | 921/921 passed |
| Both full static groups; both final static followups | Ruff, formatting and strict Pyright pass |
| Twelve frozen mutants | Every mutant fails at least one new control; no old-only rejection |
| Retained-condition audit | 22 conditions; 306 independent journaled fixture absence checks; no checker timeout or auditor cleanup signal |
| Prior evidence inventory | All 264,064 paths unchanged, including symlink text |

The full comparison gate remains **rejected**. The first full failure is `test_declared_shutdown_real_child_phases[blocking-0-denied-kill]` at `test_shutdown.py:298`. Exact receipt `candidate-full-1/runs/shutdown-blocking-denied-kill-1794738607774520000.json` records WNOHANG entry 0.0003471660311333835 seconds after shutdown end 355886.63276249997. Its fixture independently reaped the child. The second preplanned passing sample does not explain or repair this observation. It is carried into k137's existing shutdown deadline obligation alongside the prior timing and expired-request failures.

## Controls and sensitivity

Thirteen malformed controls assert the exact failure text, `readiness_outcome == "failed"`, failure before the immutable readiness deadline, original bootstrap shutdown bounds, and exact-child reap. They cover 4,097 bytes without LF, immediate EOF, partial EOF, bytes after LF, individually incorrect session/PID/PGID/SID, missing SID, an extra key, duplicate key, excessive decoder nesting, and malformed JSON. The overflow control also disallows an extra zero-length read after byte 4,097, which otherwise conceals removal of the length rejection behind EOF rejection.

The fourteenth control writes a valid identity from a real child at the real monotonic cutoff. An intercepted control read enters before the deadline, waits on the actual pipe until the child's write, and returns actual bytes. The separate report pipe timestamps write admission. The control requires the real write and postparse sample at or beyond the original cutoff, exact `late worker readiness`, outcome `late`, never ready, and bounded reap. This deliberately models delayed syscall return with a test seam; it is not natural blocking-I/O evidence and does not inject a clock.

| Frozen mutation | New rejection witness |
| --- | --- |
| no-eof | immediate and partial EOF |
| no-tail | bytes after LF |
| no-session, no-pid, no-pgid | corresponding isolated wrong identity |
| no-sid | wrong SID and missing SID |
| no-extra | extra key |
| no-duplicate | duplicate key |
| no-nesting | decoder nesting guard |
| raise-cap | overflow, including prohibition on an extra zero-length read |
| failed-expired | all thirteen malformed-frame outcome assertions |
| no-late | real-cutoff frame |

`comparison-plan.json` names every exact single-site mutation. The auditor reconstructs every mutant byte for byte from `inputs-candidate-v5`, verifies all other frozen files are unchanged, and checks that every failure belongs to a new control, with no collection errors or timeout. These executable counterexamples were the doubt-driven check; no extra same-source or cross-model reviewer was invoked.

## Decoder limitation and retained failed hypotheses

`candidate-controls-1` through `candidate-controls-4` each retain 13/14 passes with the nesting expectation failing. The native scanner decoded 1,100 and 2,040 arrays and proceeded to identity rejection. Restoring Python recursion limit 1,000 did not change this. Independent retained probes (`decoder-probe-2.stdout`, repeated in `decoder-probe-3.stdout`) decode 5,000 arrays (10,001 bytes) at Python limits 100/1,000/3,000; 10,000 arrays (20,001 bytes) exhaust the native decoder. These observations do not establish native recursion exhaustion within the 4,096-byte worker frame cap.

The final nesting control uses a real child writing 2,202 bytes and explicitly selects the standard library's Python JSON scanner with recursion limit 1,000. It verifies conversion of a real decoder RecursionError to the declared failure through that dependency seam. It makes no claim of naturally reachable native C-decoder exhaustion. This qualification is retained in the test receipt and task decision log; it is not evidence of a runtime defect.

All preliminary inputs, results and seven instrument revisions remain. The fourth unchanged run followed an ambiguous edit-script assertion and a tool lint failure. Auditor-only lint later rejected a long explicit failure key; that was corrected without runtime or test changes. Initial decoder-probe import failure and Task's embedded-shell rejection of `set -C` were reported; their saved text is explicitly reconstructed where raw capture was unavailable. The final probe task uses `/bin/sh -C`, requires a fresh OUT, and its retained negative overwrite check proves nonzero exit with unchanged output SHA-256. The shell failure did not change measured runtime sources.

## Reproduction and receipts

Run from the repository root through `mise exec -- task`. `diagnostic:startup-frames-prepare INPUTS=. OUT=evidence/startup-frame-controls-k139/inputs-repeat` freezes top-level Python sources, manifest, lock and root Taskfile; optional MUTANT names the exact single-site change. Task variables INPUTS and OUT resolve from `experiments/exec-reliability`, for example `INPUTS=evidence/startup-frame-controls-k139/inputs-candidate-v5`. Use `diagnostic:startup-frames-check INPUTS=... OUT=... MODE=controls` (new tests only), `MODE=full`, or `MODE=static`. Every condition directory must be new. Use `diagnostic:startup-frames-probe OUT=...` with a new output file. The prepare task supports the four older k134 mutations for compatibility; they were not rerun or counted here.

The condition runner freezes subjects before/after: exact inputs/config/lock/Taskfile, current task definitions, instruments, plan/deviations, spec, interpreter and tool binaries. It retains command declarations, stdout/stderr, JUnit, checker return/reap/timeout receipts, fixture identity/absence checks and artifact hashes. Focused/full/static budgets are 60/120/30 seconds with outer TERM 2 seconds then KILL/reap 2 seconds. No timeout is accepted. Independent fixture checks cover the journaled PID/PGID/start/command identities; their count is not a claim that every process in the entire full suite is separately journaled.

`audit-summary.json`, `audit.stdout` and `audit.stderr` retain the successful audit; its exclusive output prevents replacement. Historical subject hashes resolve to exact archived instrument revisions and frozen Taskfiles, never a changed-under-run allowance. `prior-before.json` and `prior-after.json` retain the full preserved inventory. `report-manifest.json` binds the final report, audit, probes and current instruments. Final task-only probe corrections and the added probe static coverage were checked in fresh static conditions after the full samples, without rerunning or replacing those samples.

Graph-assisted source verification used generation `2026-11-15T02:19:52Z`, Tier 2. Coverage metadata reported changed source and untracked new tools; direct source reads supplied the relevant evidence. Graph results are not a completeness claim.

## Remaining scope

This integration repairs startup control sensitivity only. Worker.start remains synchronous. Shared production scheduling, teardown overlap, the historical three parent timing failures, expired-request residual-remedy failure, this additional shutdown observation and final bounded-inspection-k28 review remain in complete-worker-scheduler-k137. k28 and k17 stay open; k18 retains storage work; guest acceptance remains 0/10 unavailable. No guests, privilege changes, installations or unrelated-process cleanup were used.
