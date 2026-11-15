# Expired-request ownership control (k137)

The expired-request test now follows actual exact-child ownership. A first owed
`waitpid(pid, WNOHANG)` may collect an exited worker even after the request
deadline. A residual remedy is required only when that check retains ownership.
Runtime sources are unchanged. `source-comparison.json` confirms all 13 live
Python files match the frozen candidate and only `test_worker_capture.py`
differs from the frozen parent (the parent's test came from `jj @-`).

The control uses three real schedules: command EOF followed by a non-consuming
wait for collectable exit, confirmed SIGSTOP, and an unprepared race. It records
the real wait result, requires exactly one exact WNOHANG, checks worker and
request residuals, and rejects renewed fork/signal/reap authority on repeated
requests, close and stale callbacks. Fixture reconciliation resumes the stopped
child, independently collects retained exact children and checks ECHILD.
Outer audits use `/bin/ps` after the checker returns.

Python's [waitid documentation](https://docs.python.org/3/library/os.html#os.waitid)
records macOS support from 3.13; [WNOWAIT](https://docs.python.org/3/library/os.html#os.WNOWAIT)
leaves status available to the runtime's first wait. The exited preparation
therefore explicitly skips on project Python 3.12.15 and runs under isolated
Python 3.15.1. The project environment was not replaced.

## Retained samples

| Sample | Passed / failed / skipped | Interpretation |
| --- | --- | --- |
| red | 2 / 1 / 0 | Kqueue exit notification preceded collectable status; preparation failure, not the intended RED. |
| red2 | 2 / 0 / 1 | Python 3.12 capability skip; not RED. |
| red3 | 2 / 1 / 0 | Python 3.15 already-exited case rejects the old unconditional residual assertion. |
| candidate-controls | 3 / 0 / 0 | Python 3.15 covers both fixed outcomes and the race. |
| candidate-controls-py312 | 2 / 0 / 1 | Project interpreter capability boundary retained. |
| parent-full | 917 / 4 / 0 | Python 3.15; reader subsequently failed on the legacy journal shape, so no final verdict or subject-after claim. |
| candidate-full | 920 / 3 / 0 | Python 3.15; broader timing failures and a pyright instrument-import failure retained. |
| parent-final | 920 / 1 / 0 | Serial Python 3.12; original residual assertion failed naturally. All statics passed. |
| candidate-final | 922 / 0 / 1 | Serial Python 3.12; Ruff, formatting and strict pyright all passed. |

`comparison-final2.json` compares individual JUnit items: all 920 common items
have unchanged outcomes. One old test is replaced by three schedules. This is
behavioral evidence, not a latency or performance-equivalence claim.

## Rejecting controls

| Frozen regression | Rejected schedules |
| --- | --- |
| no-first-reap | exited, stopped, natural |
| repeat-reap | stopped, natural |
| no-residual | stopped, natural |
| false-residual | exited |
| expired-signal | stopped, natural |

Every mutation is applied at exactly one checked source site. All five fail at
their intended assertion. Some mutation runner processes overlapped after the
command tool yielded; these are deterministic assertion controls and make no
timing or non-interference claim. Original no-residual and expired-signal audits
each retain two `/bin/ps` timeouts as uncertainty. The separate serial
supplementary audit observes all four PIDs absent, plus 28 fixtures from the
incomplete parent-full audit: 32 absent, no signals. These later observations
do not replace the original rejected or incomplete samples.

## Failures and limits carried forward

The initial Python 3.15 parent-full failed blocked-cleanup disposal, expected
interruption continuation, real group-snapshot helper capture, and one
competing-turn fixture's `/bin/ps` timeout. Candidate-full failed
`test_review_group_kill_precedes_inspection[hang]`, `[ignore-term]`, and
`test_review_later_zombie_table_cannot_repair_earlier_eperm`. Their exact
assertions and receipts remain in the respective stdout/JUnit/runs files.
Candidate-full began before parent-full's audit returned, so this pair is not
a serial comparison. Later passing samples explain or repair none of these
runtime failures; all remain k140 obligations alongside prior failures.

The first reader assumed the new journal schema and raised `KeyError:
'before_audit'`. The first full static checker could not resolve the sibling
audit import from the frozen working directory. The audit now accepts legacy
journals and is inline in the runner. `initial-instruments/` retains those
versions. The comparison tool's first static check reported 16 missing inferred
types; explicit collection annotations fixed them. Earlier comparison versions
are retained; the final version hashes every selected input before reading it
and after auditing. The four final tools pass Ruff, formatting and strict
pyright. The full candidate's tested runtime/test sources remain unchanged.

All completed checker verdicts report stable measurement subjects. The failed
parent-full reader has no complete verdict. `prior-after.json` verifies all
268901 prior evidence paths unchanged, including their exact inventory.

Reproduction uses `mise exec -- task diagnostic:expired-request-{prepare,check}`
with fresh output paths, `MODE=controls|full|static`, and optional `PYTHON=3.15`.
Mutants are named by `MUTANT`. `diagnostic:expired-request-verify OUT=<fresh.json>`
repeats the item comparison and supplementary absence observations;
`diagnostic:expired-request-tools` checks the tools. Every original sample is
retained; no best-run selection is made.

This control repair does not deliver concurrent production scheduling, teardown
overlap or aggregate acceptance. k140 retains the complete original charter and
historical handoffs. k28/k17 stay open; guest acceptance remains 0/10 unavailable.
An independent review must assess the test oracle, real fixture transitions,
closed authority and evidence limitations before relying on this slice.
