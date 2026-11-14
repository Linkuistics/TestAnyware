# complete-worker-scheduler-k103

**Integrates:** complete-worker-scheduler-k102

## Goal

Triage the findings of the k102 review of the k98 persistent worker close escalation. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the findings in `experiments/exec-reliability/evidence/worker-close-k102/report.md` and in the k102 commit. Re-derive the line positions from the tree as it now stands. The artifact is `Worker.close` and the seven close modes in `experiments/exec-reliability/`, with its evidence in `evidence/worker-close-k98/`. The k28 brief chain and `docs/specs/diagnostic-process-inspection.md` bind the work.

The review found no runtime defect. It found three low-severity test-strength gaps.

- F1: no mode executes denied-signal history or a PID-only target.
- F2: the `expired` mode cannot detect a post-cutoff reap probe.
- F3: no test covers a command-close failure.

Keep the slice narrow. The plan declaration of the orderly-close grace, shared scheduling, overlap with controller teardown and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k101. Preserve every prior evidence byte.

## Done when

- Every k102 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log.
- Each accepted test change has a mutation that fails it, retained as frozen evidence.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k101 and the parent aggregate open.

## Notes

The k102 review was inspection-only. It executed no mutation. Prove each new control with a mutation that the control rejects.

## Decisions (running log)

- Accepted F1, F2 and F3 from k102 commit 6abfc3ce as real test-strength gaps. Runtime inspection re-derived the existing denial-history, PID-target, cutoff and close-error behavior; no production change is needed. Add denied TERM/KILL history, an actual pre-ready child, an exited-but-unreaped expired child and a command-only close fault. Prove each with isolated frozen mutations; leave k101 and the aggregate open.
- For F2 use Darwin NOTE_EXIT without waitpid before close, so an actually exited exact child remains waitable and pins its identity. For F3 require TERM before the 50 ms orderly grace and retain the close-error text; moving the failure adjustment before kill_at alone is behavior-equivalent under its midpoint cap, so use omission of the adjustment as the mutation.
- First green-control run exposed a fixture cleanup error: Darwin returns EPERM for killpg on the already-exited group leader. Keep the actual NOTE_EXIT observation and skip independent cleanup signalling only when exit is confirmed; still require exact-child WNOHANG reap and TERM status. Preserve the failed condition and rerun with a new frozen snapshot. The first five mutations all failed their intended assertions with unchanged subjects and reaped checkers.
- The corrected frozen controls pass all 21 cases. All five corrected-fixture mutations fail their intended assertions (denied TERM, denied KILL, group before ready, post-cutoff reap, close-error grace); every checker is reaped without timeout and every subject digest is unchanged. Final full checks are running against the same corrected inputs. Prior evidence stays byte-identical; the new runner/task live under k103.
- Final full checks pass all 592 tests, Ruff lint/format and pyright against the corrected frozen inputs. All 21 control case identities match the full run, all five mutants fail their intended assertions, subjects and raw artifact hashes verify, and all 27 final close-child audits report reaped. Final process inspection finds none of the recorded checker or close-child PIDs, including failed initial attempts. The frozen report is evidence/worker-close-k103/report.md; no prior evidence changed. k101 remains live, so no parent cascade closes.
