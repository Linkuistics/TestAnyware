# Readiness review integration (k136)

The common driver now catches `OSError` and `ValueError` from advisory readiness sampling and `select`, then uses its existing deadline-capped sleep before dispatching owners again. This prevents an advisory failure from escaping worker failure retirement before KILL, exact-child reap and receipt construction. Exceptions from owner `advance` callbacks still propagate. No deadline, signal authority or registration changes were needed.

This integrates both findings from review commit `bb6d9d548b540bb5fead952a0e920695f9965e83` (`complete-worker-scheduler-k135`). F1 is a real runtime issue. F2 is a real test-sensitivity gap: the existing command-empty branch, stream deletion and cleanup-open guard already close their interests, but the previous controls did not observe those states.

Production owners still run sequentially. Concurrent production scheduling, worker-retirement/resource-teardown overlap, every remaining aggregate control, historical failures and final independent k28 review remain in `complete-worker-scheduler-k134`. k28 and k17 stay open; ten-clone guest acceptance remains **0/10, unavailable**. No guests, privileges or installed applications were changed.

## Controls and mutations

Four new exact-clock cases inject both exception classes at both advisory sites while driving the actual production retirement callback. Simulated fork, wait and signal operations prove TERM followed by KILL, reap within the original deadline, the worker receipt and removal from the owner registry. The controls require bounded sleeps and reject retry spinning. These simulated authority checks do not establish real process timing.

Two request cases observe the standalone and persistent registrations after a complete command write and after stream EOF. The command writer disappears when the command empties; after both streams close, reader and writer interests are empty. A separate retirement case observes empty cleanup interests after EOF while the owner is still alive, then verifies timer-driven KILL/reap. The existing owner-error control continues to require one propagated `OSError` with no retry or sleep.

| Frozen mutation | Rejected cases / selected cases |
| --- | ---: |
| Remove advisory fallback | 4 / 7 |
| Keep EOF streams registered | 2 / 7 |
| Keep command-writer interest after the complete write | 1 / 7 |
| Keep cleanup-reader interest after EOF | 1 / 7 |

The auditor verifies that each mutant changes exactly its declared source site and that every other frozen input matches the candidate. The seven case identities match both passing focused runs and both full suites.

## Retained measurements

`inputs-red/` freezes the reviewed driver with the first controls; `inputs-candidate/` retains the first implementation. The original RED is four advisory failures and three closure passes. The first candidate completes retirement but fails four test lower-bound comparisons: `10.9625 <= 10.962499999999999`. The corrected control calculates that lower bound from the declared inputs, `10.95 + 0.05 / 4`, without changing the reserve or upper cutoff. The initial inputs, failed outputs and six initial instruments remain intact in this corpus.

`inputs-red-final/` uses the original driver with the corrected controls. `inputs-final/` contains the final candidate. They differ only in `turns.py`; their other Python files, manifest, lockfile and task recipes match. Final measurement subjects were frozen before running and unchanged afterward. The final full candidate pair ran before the frozen parent comparison; these are sequential samples, not an interleaved pair or a production latency distribution.

| Condition | Passes / cases | Pytest elapsed | Static checks |
| --- | ---: | ---: | --- |
| red-controls | 3 / 7 | 0.49 s | not selected |
| candidate-controls-1 | 3 / 7 | 0.43 s | not selected |
| red-controls-final | 3 / 7 | 0.48 s | not selected |
| candidate-controls-2 | 7 / 7 | 0.41 s | not selected |
| candidate-controls-3 | 7 / 7 | 0.06 s | not selected |
| candidate-full-1 | 903 / 903 | 73.95 s | pass |
| candidate-full-2 | 903 / 903 | 72.52 s | pass |
| parent-full | 899 / 903 | 75.06 s | pass |

The parent full run fails exactly the four new advisory-error cases and passes the other 899 cases. Both candidates pass all 903 cases, Ruff, formatting and strict Pyright. The direct four-instrument checks also pass. Every failed condition above and all four rejected mutants remain retained; passing samples do not resolve historical timing failures.

## Evidence integrity and handoff

Use the repository's `diagnostic:readiness-review-{prepare,check,preserve,tools,audit}` tasks through `mise exec -- task`. Focused checks have a 60-second bound, full checks 120 seconds and each static checker 30 seconds, followed by bounded TERM/KILL/reap cleanup. These checker bounds do not extend runtime owner deadlines. Each condition retains command arguments, raw streams, JUnit identities, elapsed times, checker exit/reap receipts, recorded fixture audits, output hashes and before/after subject hashes.

Subjects include all frozen Python inputs, manifest and lockfile, both task recipes, the four instruments, comparison plan, observed deviations, inspection specification, interpreter and command binaries. `instruments-initial/` preserves the original instruments before the assertion repair. `condition-audit.json` verifies those exact historical hashes, every artifact hash, all declared mutation bytes, the current source/input correspondence and every parent/candidate JUnit item. It reports integrity verified and explicitly leaves aggregate acceptance false.

All checker children were reaped without timeouts. All 27 recorded exact-fixture audits report absence without cleanup signals. This covers the runner's recorded fixtures, not unrecorded descendants. `prior-after.json` confirms all 248,391 prior evidence paths retain their bytes or symlink targets; the new corpus and generated caches are excluded from that inventory.

The original k132 three parent workload timing failures and final-parent expired-request `residual_remedy` None failure remain unresolved. Earlier unknown ps/missing PID, unsupported `?E`, lost-result errno 35/exit 70/incomplete-frame/missing-EOF, deferred-reap postcutoff and helper timeout/postcutoff evidence also remains binding. Privileged real ps KILL and UID EPERM fixtures remain unavailable; storage-stall work remains k18. The intact aggregate charter and original failure details stay in k134 and the unchanged earlier corpora. This integration discharges F1/F2 only.
