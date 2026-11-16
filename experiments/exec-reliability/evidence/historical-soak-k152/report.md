# k152 historical failure soak

## Result

Five of the six open k17 cases did not recur in 46 instrumented runs. This
result makes no claim of cause. The sixth case, checker-ceiling-k77, has a
mechanism from retained timestamps and is outside the soak claim. One new
failure occurred in run 42. Its timestamps show that the fixture deadline
passed during a gap in the parent's observations. They also show a fixture
defect that leaked one process. A shared host pause is an inference, and its
cause is unknown.

The verdicts are in [verdicts.json](verdicts.json). They replace the k151
verdicts and record the k151 file hash.

## Plan and run

The plan in [plan.json](plan.json) was frozen before the first run. Its sha256
prefix is `13558d5168b3d2a0`. Workload `targeted` runs the four test functions
of the open cases, which are 7 tests. Workload `full` runs the whole suite.
Runs alternate idle and loaded, idle first. A loaded run adds 32 busy Python
processes on 16 logical CPUs.

The soak ran once into `soak/`. It completed all 46 runs in 811.5 seconds,
inside the 2400-second budget. No run was left unstarted, and no run was
retried. The hashes of the inputs, tools and plan were the same before and
after the soak.

| Condition | Runs | Seconds per run |
|---|---|---|
| targeted, idle | 20 | 6.55 to 6.74 |
| targeted, loaded | 20 | 6.73 to 6.92 |
| full, idle | 3 | 84.0, 84.3, 83.6 |
| full, loaded | 3 | 90.4, 91.0, 91.4 |

## Timing control

Before the soak, the frozen control ran one targeted run into `control/`. Each
of its six timing records passed the order checks. The control then made 87
damaged copies, by deleting each required field and by reversing each ordered
pair. The checker detected each of the 87 copies. During the soak, no run
reported a timing problem.

The k155 review integration corrected two limits of this control. The control
never deleted a KILL, and one ordered pair, child spawn before child readiness,
is not a real order. The [k155 recheck](../historical-soak-k155/report.md)
applies the corrected rules to every retained record.

## Exposure for each historical shape

| Case | Exposure in the soak | Recurrences |
|---|---|---|
| interrupted-live-group-k145 | 46 interrupted live-exit records, none with KILL | 0 |
| exited-leader-grace-k132 | 138 TERMed records, leader exit 34.3 to 60.5 ms after TERM, grace 150 ms | 0 |
| prebuffer-k106 | 92 prebuffer witnesses, helper start to end bound at most 10.2 ms | 0 |
| helper-capture-deadline | 1260 real `/bin/ps` group helpers, longest 23.7 ms, window at least 87.5 ms. No JUnit failure is a fixture `/bin/ps` timeout (k155). | 0 |
| empty-pid-k85 | 138 TERMed records, each with the PID printed at least 215 ms before TERM | 0 |

The interval statistics for each condition are in `soak/analysis.json`. Load
added up to 19 ms to the median spawn-to-readiness time. It changed the median
TERM-to-exit time by less than 1 ms.

## Run 42: deadline exhaustion and fixture leak

Run 42 was a full, idle run. Two tests failed. The first was
`test_worker_shutdown_deadline_closes_owner_operations[stale-kill-group]`. The
fixture deadline was 0.3 s after test start.

The parent made 40 nonblocking polls about 6 ms apart. The last poll came 64 ms
before the deadline. The parent then made no logged syscall until after the
deadline. The receipt finished 140 ms after the deadline.

The soak load sampler runs in another process. It woke 161 ms late in the same
window. Its next-largest delay in 808 samples was 12 milliseconds. The two
gaps overlap. A one-second sample cannot place its delay inside its interval,
so it cannot show that both processes paused together. A shared host pause is
therefore an inference, and its cause is unknown.

After the deadline, the runtime sent no signal. That is the designed
fail-closed behaviour. The fixture child loops on `signal.pause()`, so it needs
that signal. The fixture cannot clean up a child that the runtime did not
signal. So the child stayed alive as PID 4100, and its owner record stayed in
`worker_capture._owners`. The second failure,
`test_review_worker_rejects_fork_before_creation`, found that record.

This failure is in the bound-exhausted class of the historical cases, but in
another test. It is not a recurrence of a historical shape. It does not explain
the historical cases. Leaf `worker-deadline-cleanup` owns the fixture repair.

## Resource audit

The soak tool audited each stamped peer PID and each load process. It reported
no residue. That audit did not cover worker forks of the pytest process.

A manual `ps` scan after the soak found PID 4100. Its group held only that
process, its parent was PID 1, and it started at 21:07:57. Its command line
named the run 42 JUnit file. A `SIGKILL` to its group at 21:15:53 removed it.
A later scan found no process that named the soak directory. The details are in
[resource-audit.json](resource-audit.json).

## Commands

- `task diagnostic:soak-tools` checks the tools.
- `task diagnostic:soak-control INPUTS=... OUT=...` runs the timing control.
- `task diagnostic:soak INPUTS=... OUT=...` runs the soak.
- `task diagnostic:soak-analyse SOAK=...` writes `analysis.json` in the soak
  directory.
