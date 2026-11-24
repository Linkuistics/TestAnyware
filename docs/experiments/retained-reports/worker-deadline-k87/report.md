> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Worker shutdown evidence review integration (k87)

The three findings in review `complete-worker-scheduler-k86` (commit
`ae000fc2`) are real evidence issues. This integration repairs the timing
allowance, retained provenance, and independent guard controls. The k83
runtime guards remain byte-identical; `complete-worker-scheduler-k85` keeps
its full charter and remains live. This increment does not establish the
shared scheduler or the aggregate bounded-inspection acceptance contract.

## Timing and checker allowance

The retained k81 and k83 full-suite XML files contain 424 and 432 cases.
Their case-time sums are 44.207 and 53.705 seconds. The eight added deadline
controls consume 2.421 seconds; the 424 shared cases account for another
7.077 seconds. Thus the earlier report's eight 0.3-second controls cannot
explain the whole increase. The largest shared increases include controller
cutoff (0.523 seconds), missing-exit observation (0.489), inspection stdout
(0.467), real-ps failure (0.453), finally disposal (0.408), and hang (0.371).
The exact per-case deltas are retained in [audit-1/verdict.json](audit-1/verdict.json).

Sequential frozen replays used the same runner, interpreter and 90-second
outer allowance. The first pair exposed an existing relocation assumption:
`self_check.py` looks for `HERE.parents[1] / "Taskfile.yml"`. The second pair
changes only that metadata lookup to `HERE / "Taskfile.yml"` and retains a
copied Taskfile beside each snapshot. Both original failed runs remain here.
No test deadline or worker cleanup bound was changed for the timing replays.

| Replay | Cases | Failed | Pytest elapsed (seconds) |
| --- | ---: | ---: | ---: |
| [timing-k81-1](timing-k81-1/verdict.json) | 424 | 1 | 46.890 |
| [timing-k83-1](timing-k83-1/verdict.json) | 432 | 7 | 75.019 |
| [timing-k81-2](timing-k81-2/verdict.json) | 424 | 1 | 54.891 |
| [timing-k83-2](timing-k83-2/verdict.json) | 432 | 0 | 50.720 |

In the relocated pair, the eight added controls again consumed 2.416 seconds,
while shared cases moved by -5.847 seconds. The reversal and the retained
scheduling-sensitive failures show that shared process tests vary materially.
These exploratory runs are not a controlled estimate of a constant regression;
failed replays are not green verification. Their individual failures follow:

- `timing-k81-1`: `test_controller::test_review_checker_unreaped_timeout_still_retains_verdict`.
- `timing-k83-1`: `test_controller::test_review_checker_unreaped_timeout_still_retains_verdict`.
- `timing-k83-1`: `test_controller::test_review_fork_handler_reaps_gracefully`.
- `timing-k83-1`: `test_controller::test_review_fork_publishes_child_before_stdout_and_releases_on_failure`.
- `timing-k83-1`: `test_controller::test_review_live_group_is_failure_even_with_expected_leader_exit[False]`.
- `timing-k83-1`: `test_controller::test_review_live_group_is_failure_even_with_expected_leader_exit[True]`.
- `timing-k83-1`: `test_controller::test_review_pinning_control_detects_real_early_reap[None]`.
- `timing-k83-1`: `test_controller::test_review_pinning_control_detects_real_early_reap[poll]`.
- `timing-k81-2`: `test_controller::test_review_real_ps_failure_disables_later_queries_and_probes`.

The normal self-check pytest allowance is now **120 seconds**, replacing 60.
The largest observed exploratory replay was 75.019 seconds, leaving 44.981
seconds (37.5%) against that allowance. The final successful full check below
leaves 72.753 seconds (60.6%). This is measured headroom on this host, not a
hard scheduling guarantee. Only the outer suite allowance changed: per-case
workload, worker and helper deadlines and checker TERM/KILL/reap bounds remain
unchanged. A future overrun still fails and retains its evidence.

## Independent guard controls

The parameterized real-owned-child control now has ten cases: PID/group scope
for final reap, stale TERM, stale KILL, equality callbacks, and cleanup-loop
equality. For the new cleanup-loop case, the clock reaches the original
shutdown cutoff when the actual cleanup loop enters KILL. The original signal
entry and the following reap entry must both refuse the operation at equality.
The clock is restored before a separate bounded exact-child audit.

Frozen mutations remove exactly one guard each. The auditor compares every
snapshot file, proving that each mutant differs only by its named guard:

| Condition | Result | Observed forbidden operation |
| --- | --- | --- |
| [signal-red-1](signal-red-1/verdict.json) | 8 expected failures, 2 passes | real `kill`/`killpg` at or after cutoff; no late `waitpid` |
| [reap-red-1](reap-red-1/verdict.json) | 10 expected failures | exact-child `waitpid(WNOHANG)` at or after cutoff; no late signal |
| [green-1](green-1/verdict.json) | 10 passes | no late operation |

Each RED failure hits the late-operation assertion, including both new
cleanup-loop equality cases. Final-reap correctly does not distinguish the
signal mutant. All 50 deadline-control child audits across the three focused
conditions and two full checks retain exact wait status and real TERM or KILL
termination. Simulated withheld status is not kernel unreapability. A clock
sample before a system call cannot exclude preemption or kernel-call latency.

## Full verification and reconciliation

[full-1](full-1/verdict.json) passed lint, formatting and types, then passed
433/434 tests in 47.314 seconds. Its sole failure was
`test_worker_capture::test_late_created_identity_rejects_invalid_fields[token]`,
with `assert (False)` on its reaped-child assertion. This failed run remains
failed. With no source changes, [full-2](full-2/verdict.json) passed lint,
formatting, types and **434/434 tests** in **47.247 seconds**. These runs
demonstrate an intermittent existing control failure, not its resolution.
The remaining k85 implementation and aggregate review retain responsibility
for the full controller/worker contract and reliable full controls.

The [auditor](audit.py) reconciles every case identity, status and duration;
actual signals, wait selectors and wait statuses; original cutoff equality;
source and artifact hashes; mutant differences; timing deltas; and previous
evidence bytes. It uses archived green sources for the full-check subjects,
so later edits to live source do not invalidate reconciliation. External
executables retain their recorded identities and hashes; the auditor does not
require today's executables to match those historical bytes. Absolute evidence
paths remain tied to this checkout location.

[audit-1](audit-1/verdict.json) passed, checking **90,462 prior file entries**
with **zero differences**. The inventory covers file bytes, including targets
of file symlinks; cache paths, symlink text and directory symlinks are excluded.
[audit-check-2](audit-check-2/verdict.json) retains the Taskfile audit invocation,
stdout/stderr, unchanged auditor hash, 60-second outer bound and exact-child
reap. The first audit invocation, [audit-check-1](audit-check-1/verdict.json),
stopped at a Ruff line-length failure; its receipts remain, and the formatting
was corrected before the successful audit. No runtime source changed.

## Historical audit qualification and version-control evidence

The [k83 report](../worker-deadline-k83/report.md) and audit are preserved
byte-for-byte. Its audit compares archived green inputs with live sources,
except the documented four executable-metadata lines in `self_check.py`.
That reproduction assumes the producer state at **commit `c6844b2`** and is
not a valid instruction for running against arbitrary later live edits.
This report qualifies that historical reproduction; the new frozen-source
auditor above supplies the current reconciliation.

The k83 report's earlier `jj` comparison claim lacked a retained command and
output. [prior-path-comparison.json](prior-path-comparison.json) now records
the actual command comparing `da94780a` to `c6844b2a` for the complete
`probe-intervals-k81/**` and `result-framing-k80/**` paths. It returned zero,
with zero stdout/stderr bytes. The empty
[diff](prior-path-comparison.diff) and [stderr](prior-path-comparison.stderr)
are retained. This demonstrates no changes in those two bounded path scopes
between those commits; the 90,462-entry inventory independently covers prior
file bytes through this integration.

## Reproduction

Run at this checkout location, using fresh output names every time:

```sh
DIAGNOSTIC_AUDIT=audit-2 mise exec -- task diagnostic:resource-audit \
  AUDITOR=experiments/exec-reliability/evidence/worker-deadline-k87/audit.py

mise exec -- task diagnostic:worker-evidence-check \
  INPUTS=experiments/exec-reliability/evidence/worker-deadline-k87/green-inputs \
  OUT=experiments/exec-reliability/evidence/worker-deadline-k87/green-replay-1

mise exec -- task diagnostic:self-check \
  OUT=experiments/exec-reliability/evidence/worker-deadline-k87/full-replay-1
```

The focused recipe also accepts `signal-mutant-inputs` or `reap-mutant-inputs`
(expected failure), and `MODE=full` with either relocated timing snapshot.
The normal full self-check runs live subjects and freezes their hashes anew;
the archived outputs above remain the evidence for this integration.
