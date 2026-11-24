> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Worker-close review integration — k103

All three findings from k102 commit `6abfc3ce` are accepted as test-strength gaps.
Production `worker_capture.py` is unchanged. The corrected controls pass, and
five mutations fail at the intended assertions. Final full checks pass all 592
tests plus Ruff lint, Ruff format and pyright.

## Findings and repairs

- **F1 — accepted:** seeded `EPERM` TERM and KILL history now exercises the
  existing retirement ledger. A denied TERM is not repeated; KILL occurs around
  +0.20 s under the retained cutoff. A denied KILL permits no later signal and
  retains its unreaped obligation. These events are seeded history, not an
  actual kernel denial. A separate real TERM-ignoring child never publishes
  ready identity and must receive exact-PID TERM then KILL, at about +0.05 s
  and +2 s. Its signal calls and independent exact-child cleanup are retained.
- **F2 — accepted:** the expired fixture is now TERM-responsive. Darwin
  `NOTE_EXIT` observes actual exit without consuming wait status before the
  deadline expires. Close must leave the waitable child unreaped, registered
  and carrying a residual remedy; independent cleanup then consumes its TERM
  status. This makes removal of the reap deadline guard observable.
- **F3 — accepted:** only `os.close(command)` raises the controlled `EIO`.
  The test requires the retained command-close error, TERM before 40 ms and
  KILL between 1.9 and 2.05 s. Removing failure's immediate-TERM adjustment
  fails this check. Moving that adjustment ahead of `kill_at` is equivalent
  under its midpoint cap, so it is not used as a claimed failing mutation.

## Frozen verification

| Condition | Cases | Passed | Expected failures |
| --- | ---: | ---: | ---: |
| `final-controls` | 21 | 21 | 0 |
| `final-full` | 592 | 592 | 0 |
| `ignore-denied-term-final` | 1 | 0 | 1 |
| `ignore-denied-kill-final` | 1 | 0 | 1 |
| `group-before-ready-final` | 1 | 0 | 1 |
| `post-cutoff-reap-final` | 1 | 0 | 1 |
| `close-error-grace-final` | 1 | 0 | 1 |

Every final condition has equal subject hashes before and after, every checker
was reaped without timeout, and all retained artifact hashes were independently
verified. [verification.json](verification.json) records per-case results,
checker bounds, signal timing and cleanup audits. All 21 control case identities
also pass individually in the full run. Each final condition directory retains
its declaration, verdict, JUnit, stdout/stderr and raw runtime records.

The five exact edits and selections are in [mutations.json](mutations.json):

| Mutation | Observed failure |
| --- | --- |
| ignore-denied-term | TERM history repeats: TERM, TERM, KILL |
| ignore-denied-kill | KILL history is followed by new TERM and KILL |
| group-before-ready | Recorded group targets differ from required exact PID |
| post-cutoff-reap | The expired owner is reaped by close |
| close-error-grace | TERM occurs after about 51 ms instead of before 40 ms |

Final sources are frozen in `final-inputs/`; each `*-final-inputs/` differs
only by its declared runtime mutation. They match the live source bytes apart
from those intentional mutations. Earlier k103 attempts are also retained:
`green-inputs/`, `green-controls/`, `full/` and the first mutation conditions.
The first green/full attempt failed only because Darwin returned EPERM when
fixture cleanup signalled an already-exited group leader. Cleanup now skips
signalling after confirmed NOTE_EXIT and still requires exact-child WNOHANG
reap. Both initial checker processes exited and their recorded child PIDs were
absent from the final process snapshot; those failed attempts are not acceptance
evidence.

There are 11 independent close-child audits in each final green condition and
one in each final mutant, all reaped. Runtime evidence is captured before
independent cleanup: expired and denied-KILL deliberately retain unreaped
runtime obligations. [post-check-processes.txt](post-check-processes.txt) and
`verification.json` show no recorded checker or close-child PID remaining after
the checks. Cleanup success does not relabel a runtime residual as success.

## Reproduction and scope

Use `mise exec -- task diagnostic:worker-close-review-check` with
`INPUTS=evidence/worker-close-k103/final-inputs`, a fresh `OUT`, and
`MODE=controls` or `MODE=full`. Mutants use their matching final input directory,
`MODE=controls` and the `SELECT` from `mutations.json`. The runner hashes the
sources, manifest, lock, Taskfile, itself, interpreter and relevant utilities.
It caps pytest at 60 s for controls or 120 s for full, other checkers at 30 s,
then allows bounded TERM/KILL and exact checker reap on timeout.

No earlier evidence file changed. This local macOS check adds no live or
privileged VM evidence. `complete-worker-scheduler-k101` and the parent
aggregate remain open; Plan declaration of orderly grace, shared scheduling,
controller overlap and final k28 review remain their work. No new runtime
contract or ADR decision is introduced by these test repairs.
