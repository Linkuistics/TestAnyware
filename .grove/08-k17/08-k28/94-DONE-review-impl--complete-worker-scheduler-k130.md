# complete-worker-scheduler-k130

**Reviews:** complete-worker-scheduler-k126

## Goal

Independently challenge the parent-side worker request transport integration
with the shared turn driver. Read the committed producer and its evidence at
`experiments/exec-reliability/evidence/request-turns-k126/`. Verify the narrowed
contract against actual production callbacks and the retained aggregate scope.
Report findings; do not fix implementation in this review leaf.

## Done when

- Trace actual persistent and standalone capture_helper query and retirement
  registrations. Challenge ordinary/safety priority with competing ready work,
  turn I/O bounds, zero-timeout readiness, terminal callbacks and wake behavior.
- Check fixed query/shutdown deadlines, original first-reap and lifetime
  admission guards, late cleanup-only identities, unknown creation obligations,
  sticky failure and persistent close against the retained k129 charter. Do not
  infer concurrent production ownership or teardown overlap from this seam.
- Inspect frozen inputs, every retained failed sample, mutation failure causes,
  external fixture identity audit, before/after hashes and per-item parent/full
  comparisons. Challenge whether excluding observer I/O preserves meaningful
  transport read-cap and scheduling evidence. Separate inherited failures from
  any regression introduced here.
- Record actionable findings with exact committed source and evidence anchors.
  If findings require implementation, insert an integrate-impl sibling directly
  before complete-worker-scheduler-k129. The full aggregate, historical failures
  and final k28 review stay owned by k129; k28/k17 remain open.

## Decisions (running log)

- 2026-11-15: Flat review before the next producer because this seam controls
  deadlines and process ownership. Independent challenge belongs in a fresh
  Grove review session. The producer session supplied no current provider
  evidence, so no Creator line is recorded.
- 2026-11-15 k130 result: three findings. F1 is actionable. F2 and F3 are visible limits. An integrate-review-impl leaf (k131) was inserted before k129. This was a source-and-evidence review. No test, build or lint command ran. No retained evidence changed. No Creator line is written, because k126 supplied no provider evidence.
  - F1 (test sensitivity, not a runtime defect). The report states wake values at `evidence/request-turns-k126/report.md:19-21`. A ready query returns an immediate wake. An idle query returns within 5 ms. Retirement returns within 1 ms. A terminal owner keeps no stale KILL wake. The control `test_turns.py:279-307` never asserts any of this. Its `advance()` returns `wake` unchecked at `test_turns.py:293-299`. A query that returns `now` after an idle poll (`worker_capture.py:1267`) would spin the CPU until the deadline and still pass. A retirement wake that keeps a past `kill_at` after the owner is reaped (`worker_capture.py:1333-1336`) would spin the same way. None of the five mutants touches a wake. The `test_controller.py:612-631` wake assertions cover the controller owners only. Fix: assert each returned wake, add a red control, and add a rejecting mutant per wake.
  - F2 (visible trade-off). The parent loop blocked in `select` for up to 5 ms and woke at once on data. The turn polls with a zero timeout, then the driver sleeps up to 5 ms (`turns.py` sleep cap, `worker_capture.py:1267`). A receipt can therefore be seen up to 5 ms late. Historical failures k114, k116 and k121 are millisecond and microsecond boundary samples. No control measures this latency, and the report does not name it. A single-owner driver has no fd wake. k129 must decide how several owners wake on readiness. Record it there. Do not add a control that restates it.
  - F3 (remaining boundary, not a defect of this slice). The persistent dispatch `_write(worker.command, command, query_deadline)` at `worker_capture.py:1112` still blocks in a 5 ms select loop until the query deadline. It runs before `run_turns` and has no turn cap. The command is at most 4096 bytes, so a full pipe is unlikely. Neither the k126 report nor the k129 handoff names it. The retirement and publication `_write` calls (`worker_capture.py:205-223`) are child-side. Record the boundary in the k129 handoff.
  - Verified, no finding. (1) Registrations. Persistent and standalone queries register `Turn(advance_query)` at `worker_capture.py:1269`. Failed-persistent and standalone retirement register `Turn(advance_retirement, safety=True)` at `worker_capture.py:1338`. `Worker.close` keeps its earlier safety turn at `worker_capture.py:575`. (2) Priority. The test places the competing safety turn second in phase 0 and the ordinary competitor first in phase 1. Two mutants flip the flags and fail (4 and 3 cases). The persistent success case has no retirement phase, so 3 of 4 cases cover retirement. (3) I/O bounds. Each query turn reads at most 2 x 32768 bytes (`worker_capture.py:1163-1169`). The retirement read is at most 4097 bytes, so `TURN_CAP` is redundant there but harmless. The test sums requested sizes, which is the stricter measure. The uncapped-result mutant fails 2 cases with `262930 <= 65536`. The terminal-read mutant fails 2 cases with `(3840, 16) == (3840, 15)`. (4) Deadlines and guards. The parent loop and the turn share the same cutoffs: `query_deadline` for the query and `deadline` for retirement. `_reap` is unchanged (`worker_capture.py:139-160`). The first WNOHANG stays owed at an expired cutoff because the `finally` block calls `_reap` before and after the driver (`worker_capture.py:1283`, `1339`). `kill_at` is one fixed `reserve / 4` allowance. Cleanup-only control parsing, unknown creation, sticky `worker.failure` and `creation_conflict` are untouched by the diff. (5) Observer exclusion. The `reap` wrapper at `test_turns.py:226-249` clears `active` only around `/bin/ps -p`. Production `_reap` calls only `os.waitpid`, so no production read is excluded. Debug sample `debug-read-budget` retains the 152,401-byte observer total that motivated the exclusion. (6) Hashes and samples. All 17 subject hashes in `candidate-full-2/declaration.json` match the committed sources (worker_capture `887fb720`, test_turns `c66bd853`, turns `f260e0b4`, inspection `a7c21f5e`, controller `805b52b9`). Each mutant input differs from `inputs-verified` at exactly one line of `worker_capture.py`. `condition-audit.json` shows 879 common identities in both pairs, with exactly the four new cases flipping from failure to pass. The red, controls-ready (4 failures) and debug-read-budget (4 failures) samples are retained. `prior-after.json` records 206230 paths and no change. The `snapshot` equals `prior-before.json`. (7) Inherited failures. The parent-full runs fail only the four new cases. No regression appears in the candidate runs. The k85, k106, k114, k116, k121 and k122 failures stay aggregate obligations.
  - Limits, not defects. Production still drives one owner per call, so the priority controls rely on the monkeypatched driver. This slice does not show concurrent ownership or teardown overlap. The real privileged ps KILL and UID EPERM controls stay unavailable. Guest acceptance stays 0/10 unavailable.
  - Graph coverage. This review read the committed source and evidence directly with git, grep and Python JSON reads. It made no repository-wide claim from the code graph.
  - Continuation. k131 triages F1 to F3. k129 keeps the full aggregate charter. k28 and k17 stay open.
