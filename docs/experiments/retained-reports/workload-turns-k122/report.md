> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Incremental workload wait turns (k122)

The production controller now exposes `_WorkloadWait.advance()`: at most one non-reaping exit observation per turn, with a fixed PID/deadline and an absolute wake capped by that deadline. Entry at or after the cutoff admits no observation; an observation returning at or after it cannot establish success. Completion and observation errors make later turns inert. The existing `_wait` caller drives these turns and sleeps outside them, retaining its 5ms maximum sleep.

This is a bounded implementation slice, not aggregate scheduler acceptance. The entire original k122 charter and history were copied to flat continuation **complete-worker-scheduler-k124** before this leaf was narrowed. k28/k17 remain open and ten-clone acceptance remains **0/10, unavailable**.

## Frozen conditions and results

The baseline is `inputs-old`; the first candidate is `inputs-final`; the corrected candidate is `inputs-final-2`. Final controller SHA-256 is `ebf1d67696492162514283a32dcc95888477250580d64d626441c084e81ccace`; final test_controller.py SHA-256 is `cd8b0289e6044f0ca922fa635a6c8fa823d0291a0d20014d353fa5d7c9ba4787`.

| Condition | Result |
| --- | --- |
| RED baseline, six new controls | Six failures: missing `_WorkloadWait` |
| First candidate, all workload-wait controls | 17 pass; one existing exact-float sleep assertion fails |
| Corrected candidate, all workload-wait controls | 18 pass |
| Remove entry cutoff | Two failures |
| Remove observation-return cutoff | Two failures |
| Sleep inside advance | One failure |
| Observe twice inside advance | Four failures |
| Remove terminal guard | Six failures |
| Extend wake beyond deadline | One failure |
| Corrected candidate, full suite | **866 pass in 76.43s**; Ruff, format and strict Pyright pass |

The first candidate requested `0.0050000000000000044` rather than exactly `0.005` seconds. Restoring the original 5ms maximum alongside the absolute-wake cap fixed the compatibility consumer without changing the test. Both candidate snapshots, the failed sample and the added `green-controls-2` sample remain retained. `comparison-plan.json` is unchanged; `tooling-deviations.md` records the corrective sample and initial Taskfile insertion error. `audit.initial.py.txt` preserves the measured auditor used by the two earlier samples.

Every condition retains raw streams, JUnit, declared checker commands/bounds, source/configuration/interpreter/helper/checker hashes before and after, artifact hashes and checker exit/reap receipts. Inputs and subjects remained identical during each run. All six mutations replace exactly one site inside `_WorkloadWait`. These controls challenge the turn contract, not every possible scheduler fault. The unchanged k91 controls continue to cover compatibility behavior, including late real workload exit followed by termination and safety teardown.

Each new control owns a real direct child with a unique command token. Ready cases exercise real NOTE_EXIT observation; pending cases observe a real live child. Cutoff time and observer-error delivery are injected around that real observation. Within the turn, sleep, waitpid and signalling/probing calls are forbidden. Fixture cleanup occurs outside the patched turn through the original subprocess owner, followed by independent ps absence inspection. This is not a real delayed-kernel-observation experiment.

## Resource and preservation audit

`condition-audit.json` verifies all ten retained conditions, the expected failure cases, immutable subject/artifact hashes, final production/frozen-input byte equality, all checker exits and prior-evidence preservation.

- All **60** new workload-turn child journals report independent final absence; each is itemized under its condition's `runs/workload-turn-*.json`.
- The full checker's two owned termination fixture journals also report absence in `full/verdict.json`; no cleanup signal was needed by the external fixture auditor.
- All **13** checker invocations were reaped, with no timeout or unresolved checker residual. Bounds stayed fixed: focused pytest 60s, full pytest 120s, static checks 30s each, then 2s TERM and 2s KILL/reap cleanup if needed.
- `prior-before.json` and `prior-after.json` compare **193,709** prior evidence paths: no additions, removals or changed bytes outside this new evidence directory, excluding interpreter/test caches.

Reusable recipes are `diagnostic:workload-turns-prepare`, `-check`, `-tools`, `-preserve`, and `-audit` in Taskfile.yml, invoked through mise. The runner rejects existing output directories. Reproduction requires fresh destinations; retained outputs are immutable.

## Remaining work and limits

`_wait`, `_terminate`, `_reap_leader`, final helper capture and worker close still have synchronous compatibility drivers. Persistent capture failure also performs private helper cleanup/drain and synchronous worker close; a controller wait pump alone cannot establish failure/teardown overlap. No common scheduler or transport ownership boundary was changed here.

k124 retains pre-resource persistent-worker admission; helper launch/result protocol; sticky failure; all worker/helper/workload and teardown timers in one safety-first scheduler; actual overlap controls; shutdown/first-reap lifetime authority; all outstanding real fault controls and prior intermittent failures; interleaved parent/candidate scheduler comparisons; and final `Reviews: bounded-inspection-k28` review before k21. A green sample here does not explain or repair historical shutdown-cutoff, missing-receipt or lost-result failures. Privileged UID/ps controls and ten-clone guest acceptance remain unavailable.

Graph-assisted discovery used Tier 2 with generation `2026-11-15T02:19:52Z`. Coverage had no recorded gaps but changed-source freshness and untracked new evidence scripts required direct source verification; no graph completeness claim is made. Independent review of this seam is scheduled separately before continuation, without substituting for the final aggregate review.
