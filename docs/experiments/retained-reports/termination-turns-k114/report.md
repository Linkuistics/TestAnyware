> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Incremental workload termination turns (k114)

`controller._Termination` freezes T, K and the inspection/reap reserve before
publishing its deadline receipt. Each `advance()` performs at most one exit
notification check, never sleeps or queries a process table, and returns an
absolute wake or an idempotent terminal `None`. The caller may service other
work between turns. Group KILL remains mandatory at T even after leader exit;
PID-only accepted exit suppresses further signals and shortens K. A starved
caller gets no renewed grace or authority after the cutoff. Signal admission
and exit-return cutoff checks and sticky observation errors remain in force.

The existing `_terminate` consumer drives this owner, then performs the sole
final process-table query and existing permission/success classification. It
still synchronously sleeps and inspects. This slice does **not** implement the
shared earliest-deadline scheduler, worker/teardown overlap, or aggregate guest
acceptance. The entire aggregate charter/history remains in the flat k116
continuation, with the final k28 review still required and acceptance 0/10,
unavailable. k28/k17 stay open.

## Retained conditions

Each condition contains its commands, bounded exact-child checker receipts,
read-subject hashes before/after, raw output, JUnit and artifact hashes. Inputs
are immutable copies. No subject changed during a condition. Checker limits
are 60 seconds for selected tests, 120 seconds for full tests and 30 seconds
per static check, followed on timeout by TERM 2 seconds and KILL/reap 2 seconds.
No checker timed out or retained an unreaped child.

| Condition | Result |
| --- | --- |
| `red` | All 20 new controls fail for missing `_Termination`; 826 deselected |
| `green-controls` | 40 pass: 20 new direct/real turns plus 20 unchanged cutoff controls |
| `green-full` | 844 pass, 2 fail; all static checks pass |
| `green-full-2` | 846 pass; lint catches one overlong mutation string, format/type pass |
| `green-full-3` | 846 pass; Ruff lint/format and strict Pyright pass |
| `renew-term` | 13/20 controls reject renewing T on each turn |
| `early-group` | 3/20 reject cancelling mandatory group KILL after leader exit |
| `late-wake` | 2/20 reject waking after the next absolute boundary |
| `baseline-lost-1` through `baseline-lost-5` | 4/4 transport controls pass in each baseline sample |
| `candidate-lost-1` through `candidate-lost-5` | 4/4 transport controls pass in each final-candidate sample |

The first full run's source-mutation control still targeted the old local
`term_deadline + plan.kill_wait` expression in `_terminate`. Its failure was
an absent mutation site, not a successful behavioral challenge. The corrected
control mutates `_Termination` after the exit observation and still proves that
renewing K changes its observed capped-deadline predicate. The following lint
failure was fixed by splitting an adjacent string literal; the generated
mutation bytes did not change. All failed samples remain intact.

The other first-full failure was
`test_review_worker_fork_failure_evidence[lost-result]`. Its retained receipt
`green-full/runs/worker-fork-failure-lost-result-1794712748681548000.json`
records worker PID 21679, reaped exit 70, a valid errno 35 fork-failure report,
no helper identity/obligation and no creation conflict, but unknown creation.
The error is `worker capture failed: incomplete worker frame length`. Clean
no-child authority requires both channels to be closed; that condition was not
established. Neither worker capture nor inspection source changed in this
slice. Five interleaved baseline/candidate samples pass, as do the next two
full suites. These observations do not explain or erase the retained transport
race; k116 must account for its repeatability alongside earlier unknown-table
and late-identity failures. Conservative unknown behavior was not relaxed.

## Controls and ownership

Direct fake-clock controls forbid sleeping, table inspection and constructor
signals; cover group/PID scope, accepted early exit, exact wakes, T/K/cutoff
boundaries, starved first turns, late observations and terminal idempotence.
The compatibility consumer retains all existing cutoff and final-inspection
controls. The moved deadline mutation remains in the full suite.

Two real exact-child controls start independent process groups whose leaders
ignore TERM, advance the owner while servicing another timer, verify TERM/KILL
scope and timestamps, and retain the pinned leader observation. Their finally
blocks independently KILL only the owned PID, wait for that exact child within
2 seconds and retain a post-reap absence query. This also runs on RED and on
the named mutations, so assertion failures cannot skip fixture reconciliation.
`condition-audit.json` verifies every retained exact-child audit, all condition
artifact hashes and unchanged subject hashes. It also verifies the preexisting
evidence inventory in `prior-after.json` against 176,131 baseline paths.

The initial long-running bootstrap command completed the original RED run.
Duplicate preparation/inventory attempts were refused by exclusive creation
with `FileExistsError`; they did not overwrite the original evidence. The
original RED result and its two fixture absence receipts are retained.

## Reproduction

Use root Taskfile tasks through mise. Choose fresh output paths: preparation,
conditions, inventories and the aggregate audit intentionally refuse overwrite.

- `diagnostic:termination-turn-prepare INPUTS=. OUT=<fresh-inputs>` freezes the
  current diagnostic sources, manifests and Taskfile. `MUTANT=renew-term`,
  `early-group` or `late-wake` applies exactly one declared source mutation.
- `diagnostic:termination-turn-check INPUTS=<inputs> OUT=<fresh-run>
  MODE=controls` selects termination turns/cutoffs; `SELECT=termination_turn`
  isolates new controls. `MODE=full` runs the full suite and static checks.
- `diagnostic:termination-turn-tools -- --check` checks the four evidence tools.
- `diagnostic:termination-turn-preserve MODE=before|after` brackets evidence.
- `diagnostic:termination-turn-audit` verifies this report's retained conditions.

A separate scheduled review must challenge this bounded seam before the
continuation relies on it. No guest, privileged, installed-app or unrelated
process operation was performed.

Final audit passed across 18 conditions, 27 reaped checker children and 16 real
fixture post-reap absence receipts. All 176,131 prior evidence paths are
unchanged. Working diagnostic source bytes match `final-checked-inputs`.
Final evidence-tool format/lint/type checks also pass after the audit learned
the five baseline/candidate pairs.
