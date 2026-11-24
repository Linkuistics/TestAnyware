> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Review of the k98 persistent worker close escalation (k102)

## Result

The review found no runtime defect in the bounded k98 contract.
It found three low-severity test-strength gaps. All three are actionable, so the review cut one integrate step.

The review ran no test, build, lint or format command.
It executed no counterexample or mutation, so it froze no new subject.
It ran read-only hash and byte-comparison commands against committed files. They changed nothing.

The frozen k98 records stay as they were.

## Scope and coverage limits

The review read the k98 diff (commit `2228059`, parent `3d030cb`) and the k98 `report.md`.
It read `Worker.close` (`worker_capture.py:368-428`), `_reap` (`worker_capture.py:86-103`) and `_signal` (`worker_capture.py:106-121`).
It read every `Worker.close` caller: `worker_capture.py:363`, `:727`, `:1135`, `:1148` and `controller.py:745`.
It read the seven close modes (`test_worker_capture.py:2218-2316`), the ten cutoff controls (`test_worker_capture.py:21`) and the startup-fault test (`test_worker_capture.py:2164`).
It read the runner, the three verdicts, the three declarations, `audit-summary.json` and `preservation.json`.
It read `docs/specs/diagnostic-process-inspection.md` and the k28 brief chain.

The code graph is stale for this slice. The index generation is `2026-11-14T22:15:36Z`, before commit `2228059`.
The coverage tool reports `metadata_changed` for `worker_capture.py`, `test_worker_capture.py` and `Taskfile.yml`.
It reports `not_tracked` for `run_condition.py`.
The review used direct source reads and git. It made no claim from the graph.

It did not read every test body in the three test files.
The privileged controls, the real EPERM fixture and the ten-clone acceptance were not executed. The review treats them as unavailable.

## Close arithmetic

The healthy close, the failure close and the retiring close share one formula.

1. `Worker.close` returns at once if `closed` is set (`worker_capture.py:369-370`). A repeated close is inert.
2. `retiring` is true when the owner deadline is finite or any signal is recorded (`worker_capture.py:378`).
3. Only a healthy, non-retiring close adds the 50 ms grace to `term_at` (`worker_capture.py:379-380`).
4. Each recorded TERM sets `term_sent` and lowers `term_at` to its timestamp. A recorded KILL sets both flags (`worker_capture.py:381-386`). The code never reads `errno`, so a denied attempt counts.
5. The owner deadline becomes the lesser of itself and `shutdown_started + 4` (`worker_capture.py:388-390`). An earlier finite deadline wins.
6. `kill_at` is the lesser of `term_at + 2` and the midpoint of the remaining allowance (`worker_capture.py:393-397`).
7. A failure, including a failed command close, lowers `term_at` to `shutdown_started` (`worker_capture.py:405-406`).
8. The loop runs only while `now < deadline`. It reaps, then sends TERM at `term_at` and KILL at `kill_at`, each at most once (`worker_capture.py:407-418`).
9. A final `_reap` runs. A worker that is unreaped or exits non-zero sets the failure (`worker_capture.py:419-421`).

For the default deadline `shutdown_started + 4`, the midpoint is `shutdown_started + 2`.
A healthy close sends TERM at +0.05 s and KILL at +2.0 s. The `term_at + 2` bound of +2.05 s never applies.
A failure close sends TERM at +0 s and KILL at +2.0 s.
Step 7 runs after step 6. It cannot change `kill_at`, because the midpoint already bounds it at +2.0 s.

## Answers to the leaf questions

### Skipped, repeated or renewed signals

No path skips TERM, repeats an attempt or renews grace.
A prior denied TERM counts as sent. A prior KILL ends all signalling. A finite deadline caps the midpoint.

A failed command close removes the healthy grace and moves TERM earlier. It never delays KILL.
`_signal` and `_reap` act on the unreaped direct child only. A `waitpid` error sets `reap_errno` and ends authority.
The target is the group when `owner.ready` is true and the PID otherwise (`worker_capture.py:115-118`).

### Cutoff, reap claims and helper obligations

No close path signals or probes after the owner deadline.
`_signal` returns at `worker_capture.py:111-112`. `_reap` returns at `worker_capture.py:90`. The loop and the final `_reap` use the same cutoff.

A reap claim needs `waitpid` to return the owner PID (`worker_capture.py:100-103`).
`Worker.close` never touches `_helper_obligations`. The k99 and k100 receipts own that ledger.
The unreaped worker keeps its entry in `_owners` and the `residual_remedy` text in `evidence()`.

### Pre-existing gaps, not regressions

The plan does not declare the orderly-close grace. The spec says the plan declares it.
`Plan` carries `term_grace` and `kill_wait` for the workload only (`controller.py:56-57`).
The 50 ms value is fixed inside `Worker.close` (`worker_capture.py:380`).

The spec phrase is "orderly close grace and KILL/reap". It names no TERM step for orderly close.
The k98 design adds TERM at 50 ms. The KILL time of +2.0 s still meets the spec bound.

The k98 decision log records the 50 ms choice. The plan declaration and the TERM step belong to k101.
The close loop stays synchronous. Shared scheduling and overlap with controller teardown also belong to k101.

## Findings

### F1. No mode executes denied-signal history or a PID-only target (low, test strength)

The seven modes seed history only through `_signal`, so each recorded event is a real, successful signal.
No mode injects an event with `errno`. The k98 report says the runtime handles denied history. Only source reading supports that claim.

Every mode reaches `start()` success, so `owner.ready` is true and `_signal` always signals the group.
The startup-fault tests reach a PID-only close, because readiness never arrives. They assert only that the worker was reaped (`test_worker_capture.py:2164-2192`).

They do not assert the signal list or the TERM-to-KILL timing. Their fixture also dies on TERM, so KILL never runs.

A mutation that repeats a TERM after a denied attempt would pass all seven modes.
A mutation that skips KILL for a pre-ready, TERM-ignoring worker would also pass.
The integrate step should seed an `errno` event into `owner.signals` and add a pre-ready, TERM-ignoring fixture.
Both controls should assert the signal list and the phase bounds.

### F2. The `expired` mode cannot detect a post-cutoff reap probe (low, test strength)

The `expired` mode pre-seeds a TERM and sets the deadline to now.
The fixture ignores TERM in every mode except `term` (`test_worker_capture.py:2228-2229`). The child therefore never exits.

A `waitpid` probe after the cutoff would also find nothing. The assertion `not owner.reaped` cannot tell "no probe" from "probe found nothing".

The ten cutoff controls do cover the `_reap` guard through a simulated deferred wait. They patch `_child` and drive the one-shot cleanup loop (`test_worker_capture.py:21-91`).
They share `_signal` and `_reap` with `Worker.close`. They do not run `Worker.close`.

A mutation that removes the `_reap` deadline guard passes the `expired` mode.
The integrate step should add a TERM-responsive variant. Its child exits after the pre-seeded TERM, and the deadline has expired before close.
The control should assert the owner stays unreaped, the residual remains and the independent audit reaps the child.

### F3. No test covers a command-close failure (low, test strength)

The leaf asks whether a command-close failure can skip TERM or renew authority.
Source reading says no. The failure only moves TERM earlier and the midpoint keeps KILL at +2.0 s.
No test sets `os.close` to fail for the command descriptor. A search of the tests finds no reference to `worker command close failed`.

A mutation that applies the failure check before `kill_at`, or drops it, would pass all seven modes.
The integrate step should make `os.close` fail for the command descriptor of a healthy worker.
It should assert TERM before 0.1 s, KILL near 2 s and a retained failure text.

## Observations, not findings

- Timing slack is wide. The `term` and `kill` modes accept a first signal between 0.04 s and 0.5 s (`test_worker_capture.py:2282`). GREEN observed 50.2 ms to 51.1 ms. A grace of 0.3 s would pass. The spec allows up to 2 s, so no contract bound is breached.
- The second-signal gap bound is 2.05 s (`test_worker_capture.py:2286`). GREEN observed 1.95 s for `kill` and 2.00 s for `failure`.
- `retiring` treats any finite deadline as retirement. The request path sets a finite deadline (`worker_capture.py:845`) and resets it to infinity only on success (`worker_capture.py:1155`). Only an escaping exception can leave a finite deadline with no failure. The review did not trace every raise site. The contract says the original deadline always wins, so the result matches it.
- The audit is in-process. `os.waitpid` raising `ChildProcessError` shows the child is gone. It cannot show whether the runtime or another reaper collected it. The test asserts the runtime return code separately.
- RED and GREEN ran with runner digest `f5e09230…a4772`. Its bytes are not retained. The shipped runner is `1378139f…988`. The declared argv in `green-1` matches the shipped controls mode: the same `-k` expression and the same 60 s bound.
- The four preservation modes `orderly`, `failure`, `retiring-kill` and `expired` also passed on the unchanged runtime. They protect old behaviour and do not prove the changed lines. The k98 report claims no more.

## Evidence verification

| Claim | Check | Result |
| --- | --- | --- |
| GREEN and full subjects equal the shipped sources | `cmp` of both inputs against `HEAD` | Equal |
| RED ran the new tests on the old runtime | `cmp` of RED inputs against `3d030cb` and `HEAD` | Runtime equals parent, tests equal `HEAD` |
| RED failed three controls | `red-1/verdict.json` | `ok` false, `pytest` exit 1, 3 failed and 14 passed |
| GREEN and full pass | `green-1`, `full-1` verdicts | `ok` true. Full has pytest, Ruff, format and Pyright exit 0 |
| Subjects unchanged in every sample | `subjects_before` equals `subjects_after` | True in all three |
| Retained artifacts match | SHA-256 of each artifact against its `HEAD` blob | 38, 38 and 1718 files. None missing or different |
| Fifty-one exact-child audits | `audit-summary.json` | 51 records, 51 distinct PIDs, all reaped, none still present, 17 per sample |
| Prior evidence preserved | `git diff` from `3d030cb` to `2228059` under `evidence/` | Every changed path is under `worker-close-k98` |

The three RED failures are `[term] assert -9 == -15`, `[kill] assert ['SIGKILL'] == ['SIGTERM', 'SIGKILL']` and `[retiring-term] assert False`.
They match the changed lines: a TERM-responsive worker, the TERM step before KILL and the earlier deadline.

GREEN signal offsets from the retained runs, in seconds after `shutdown_started`:

| Mode | Signals | Finish |
| --- | --- | --- |
| `orderly` | none | 0.0015 |
| `term` | TERM 0.0511 | 0.0527 |
| `kill` | TERM 0.0502, KILL 2.0005 | 2.0021 |
| `failure` | TERM 0.0, KILL 2.0012 | 2.0026 |
| `retiring-term` | TERM before close, KILL 0.2004 | 0.2019 |
| `retiring-kill` | KILL before close | 0.0015 |
| `expired` | TERM before close | 0.0, unreaped |

Runtime success and the independent audit are separate claims.
The `expired` mode ends with an unreaped worker and a residual. The fixture then kills and reaps the child in its `finally` block, after it captures evidence.
The audit file records that cleanup. The k98 report does not attribute it to the runtime.

## Routing

The review cut `complete-worker-scheduler` as an integrate-review-impl leaf ahead of k101.
It leaves k101 and the parent aggregate open.
The future complete-runtime review of k28 stays in k101.
