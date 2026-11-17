# macOS post-timeout inspection stall (leaf macos-timeout-inspection-k4)

This report accounts for issue #14 failure A on macOS. Failure A is the stall of a
30 s guest inspection that runs immediately after an intentional 2 s timeout. The
report keeps each old failure apart from later samples and from the ordinary-command
timeout of the k3 report.

## Result

On one fresh unchanged clone, a process-listing command timed out after the
intentional timeouts. The command was not the known-child `ps -p` observation. The
listing printed 460 rows that end with its own `ps` row, and the agent then reported
a guest timeout at 30 s. The same command ran in 0.150 s three operations earlier on
the same clone.

The checker counts this call only from revision 2, which changed after the run. So
the result is a post-hoc expansion. It is not a predeclared
reproduction of the historical known-child observation, and it does not prove a
shared cause.

The evidence is incomplete. The complete original r3 fixture condition stopped at its
cancel case because of a plan defect. A second plan defect ran its online case
without the pinned URL and hash. The cause of the stall is not proved. The shape is
consistent with the k3 completed-then-timed-out mechanism, but no evidence in this
run shows when `ps` or the guest shell exited.

## Old failures, kept separate

The original failed inspections stay in the retained VM-fit workspace
`Ravel-Strategies.gh-issue-1/experiments/tutorial-vm-fit/evidence/`. This leaf reads
them and does not copy them.

| run | fixture | `observe-timeout` | receipt |
|---|---|---|---|
| `macos-01/clone-1` | r1 | 0.111 s, exit 0 | `timeout pid=696`, no `ps` row: the child was gone |
| `macos-02/clone-1` | r2 | guest timeout at 30 s | `timeout pid=709`, then `Process timed out after 30s` |
| `macos-03/clone-1` | r3 | 33.067 s, guest timeout | `timeout pid=666`, then `Process timed out after 30s` |

The `macos-01` receipt shows that `ps -p` prints nothing when the child is gone. The
`macos-02` and `macos-03` receipts hold the PID line, no `ps` row and the agent's
timeout note. A timed-out sample cannot show whether the child was absent or
whether more output would have followed. Those receipts also do not show whether
`ps` blocked or whether the command completed and the agent did not observe the
exit. The `macos-03` elapsed time of 33 s is consistent with the 30 s deadline and an
expired 3 s second wait after TERM. No phase timestamp measures those waits.

These samples are different from the ordinary-command timeout of the k3 report
(`macos-candidate-01` op 08, 30.058 s). That command was not preceded by an
intentional timeout. The retained `diagnosis-macos-01/` run did not reproduce failure A.

### Old builds that remain available

- The original runs used CLI and app 2.7.0 with Tart 2.41.1. This host holds only
  Homebrew 2.8.0 (`Cellar/testanyware/2.8.0_1` and the 2.8.0 app archive). The
  2.7.0 host side is not available.
- The guest agent of the original runs, sha256 `1282f4cf…8a84`, is the agent inside
  the retained golden `testanyware-golden-macos-27`. Run 02 recorded that hash in
  the clone. The guest side of the old path is available.
- The agent source of `ProcessRunner.swift` and `handleExec` has not changed since
  commit `868ca80308`. The k3 report records the provenance gap of the guest binary.

## Frozen plan and run

The plan `experiments/exec-reliability/timeout_k4.py` froze one clone of
`testanyware-golden-macos-27` and 36 calls for run 02, in this order:

1. `health-before`.
2. `issue-sleep`: exact issue #14 `sleep 120`, guest timeout 2 s.
3. `issue-child`: exact issue #14 fork/wait payload, guest timeout 2 s.
4. `issue-observe`: exact known-child `ps` observation, guest budget 30 s, host bound 50 s.
5. Ten after-timeout operations `after-01` to `after-10`: health, observation,
   process table, agent threads, two ordinary commands, identity, observation,
   process table, health.
6. The complete r3 VM-fit sequence, from a byte-exact copy of the fixture under
   `experiments/exec-reliability/evidence/macos-timeout-inspection-k4/fixture-r3/`.
   The plan refuses to run when a hash differs from the retained `frozen.json`.
   Run 02 did not reproduce two original invocations: the online case and the
   cancel expectation. The section "Plan defects and unfinished conditions" gives
   both.

The r3 condition runs after the issue payloads on the same clone. The original r3
sequence ran on a clone that had no earlier timeouts. A second unchanged clone would
exceed the limit of one unchanged clone for each causal leaf.

### Attempts

| attempt | outcome |
|---|---|
| `evidence/k4-timeout-macos-01` | The controller refused the dotted call names before acquisition. No clone started. |
| `evidence/k4-timeout-macos-02` | The unchanged clone. Source commit `81121010`, clean. Run `6de306c9…6979`. |

Run 02 identities: host CLI 2.8.0, guest macOS 27.0.1 arm64, 4 CPUs,
`kern.wq_max_constrained_threads` 64, guest agent `1282f4cf…8a84` as PID 421. The
host agent bundle is `22dd56ce…b9f5`. Acquisition took 16.154 s. The tool digests
before and after the run are equal. The app PID 1488 was the same before and after.

### Phases of run 02

Times start at `issue-sleep`.

| call | start | elapsed | receipt |
|---|---|---|---|
| `issue-sleep` | 0.000 s | 2.242 s | exit -1, timed out, overhead 0.242 s |
| `issue-child` | 2.242 s | 2.281 s | exit -1, timed out, overhead 0.281 s |
| `issue-observe` | 4.524 s | 0.156 s | `child pid=630`, no `ps` row, exit 1, empty stderr |
| `after-01` to `after-08` | 4.68 s | 0.14 s to 0.18 s each | normal |
| `after-03-table` | | 0.150 s | 459 rows |
| `after-09-table` | 5.808 s | 30.148 s | exit -1, timed out, 460 rows, last row `695 421 … ps -axo …` |
| `after-10-health` | 35.957 s | 0.081 s | healthy |
| `r3-online` | 37.770 s | 0.151 s | exit 1, `$2: unbound variable`: the plan omitted the URL and hash |
| `r3-timeout` | 38.150 s | 2.209 s | exit -1, timed out, overhead 0.209 s |
| `r3-observe-timeout` | 40.359 s | 0.140 s | `timeout pid=799`, no `ps` row, exit 0 |
| `r3-cancel` | 40.500 s | 2.108 s | interrupted, CLI exit -15 |

The three intentional timeouts had host overheads of 0.209 s to 0.281 s. Each
overhead includes the host transport, so each one is an upper bound on the agent's
work after its deadline. No overhead exceeded the 4 s sum of the TERM grace and the
kill/drain/reap bound, so the run records no grace or reap violation.

`issue-observe` listed no process for PID 630. Its exit 1 with empty stderr is the
`ps -p` no-match result. The run has no signal or exit record for PID 630, so it does
not show which action ended the child. The r3 observation masks a `ps` failure with
`|| true`, so its missing row for PID 799 shows only that the fixture listed no row.
The r3 cancel child, PID 812, survived the CLI interruption, as in `macos-01`. The
resource cleanup sent TERM to it by its pid file, only after it checked that the
command was `sleep 120`.

### The stalled call

`after-09-table` ran `ps -axo pid,ppid,pgid,stat,lstart,etime,command`. Its stdout
holds 460 rows. `ps` sorts by PID, and its own row (PID 695, parent 421, the agent)
is the last row. So the output is substantial.

The output does not show that `ps` finished
its enumeration or its output, because the run did not time the exit of `ps` or of
the shell. The stderr holds only the agent's note `Process timed out after 30s`. The
elapsed time of 30.148 s is consistent with a second 3 s wait after TERM that ended
almost at once. No phase timestamp measures that wait.

What this shows about the waiting operation:

- It did not wait on an earlier request. The previous request ended 0.15 s before,
  and the agent answered each of the eight earlier after-timeout calls in under 0.2 s.
- It did not wait on host transport. The host received a guest receipt that states a
  guest timeout, and the next health call took 0.081 s.
- It did not wait on descendant enumeration as the first cause. By the agent source,
  the agent enumerates descendants only after its 30 s deadline expires.
- It waited on its own child or on the agent's exit observation. Either `ps` or the
  shell did not exit. Or both exited, and the agent did not observe the exit within
  30 s. This run cannot distinguish the two cases. It also cannot exclude that `ps`
  was still enumerating or writing after its last printed row.

The table shows processes in state `U` (uninterruptible wait), for example PID 668
`hybridsearchd`, 673 `ReportMemoryException` and 674 `mdbulkimport`. These are
candidates for a `ps` exit delay. The run has no evidence that `ps` read them after
it wrote its output.

## Checker results

`timeout_k4_check.py` revision 1 was frozen before run 02. It counted only the
observation calls as possible stalls. It judged run 02 `evidence incomplete, stall
unknown` (`k4-timeout-macos-02-outer/check.json`).

Revision 2 also counts an ordinary guest command that times out, because each one
runs after an intentional timeout. It judges run 02 `evidence incomplete, stall
reproduced`, with `after-09-table` stalled (`check-r2.json`). Revision 2 changes the
judgement after the run, so the report keeps both results.

Revision 3 follows review k192. It requires a typed receipt and the expected outcome
of each role. For the r3 calls, the outcome includes the original exit codes. It
also includes the transfer paths and sizes and the pinned download bytes. A stall counts as a reproduction only with valid local
evidence and a valid earlier intentional timeout. A printed child without a `ps` row
is absent only in a valid observation of the plan's own payload.

Revision 3 judges
run 02 `evidence incomplete, stall reproduced`, with `after-09-table` admitted
(`check-r3.json`). It also fails `r3-online`, which revision 2 accepted, and
`r3-cancel`. It reports the run 02 cleanup state as `unknown`.

## Accounting

- The ledger shows the clone `k4-macos-6de306c9…6979` disposed in 1.215 s, and the
  controller named no residual process. The VM inventory after the run holds no clone.
- The resource cleanup acted only on the three known pid files. It printed an empty
  command for PIDs 630 and 799. That cleanup treated an empty or failed `ps` output
  as a skip, so this output does not show the PIDs gone. It sent TERM to PID 812 and
  printed `terminated pid=812`. It printed `cleanup complete` without a condition.
  Clone disposal is the independent guarantee, and it does not prove the earlier
  absence of any child.
- The cleanup now reads `kill -0` for each PID and checks that each signalled PID
  is gone. It ends with `cleanup confirmed` only when no PID ended unknown or
  alive. Otherwise it ends with `cleanup uncertain` and exits 1. Its guest timeout
  is now 9 s, for three children with 2 s of TERM grace and 0.5 s of KILL wait.
- No broad process-name cleanup ran. The host app was not stopped.

## Plan defects and unfinished conditions

The plan ran `r3-online` as `guest.sh online` with a guest timeout of 30 s. The
original call passed the pinned yq URL and hash from `cases.json`, with a guest
timeout of 120 s (`macos-03/clone-1/online.json`, exit 0 in 1.375 s). The guest
script stopped at `$2: unbound variable` with exit 1. That failed sample stays
retained.

The plan now passes the original argument and guest timeout, and a control checks
both. The plan now also checks agent health before the r3 sequence,
as the original run did. A control pins the r3 call order to the original clone-1
order.

The plan gave `r3-cancel` the default expected exit 0. The interrupted CLI exits -15,
so the controller marked the call failed and stopped the plan. These r3 calls did not
run: `r3-observe-cancel`, the two partial downloads, `r3-missing-artifact`,
`r3-guest-cleanup` and `r3-observe-after-cleanup`. The plan now expects exit -15, and
a control checks that value.

These conditions of the leaf remained open after run 02:

- The complete original r3 fixture condition, as a separate condition. The run 02 r3
  samples are not that condition, because two of its invocations differ. Leaf k193
  ran it later. See the section on the complete r3 condition alone.
- An instrumented clone that records when the shell and `ps` exit and when the agent
  observes the exit. Only such a run can separate a blocked `ps` from a lost exit.

## Complete r3 condition alone (leaf macos-timeout-inspection-k193)

Leaf `macos-timeout-inspection-k193` ran the complete original r3 fixture condition
once, alone, on one fresh unchanged clone of `testanyware-golden-macos-27`. No issue
payload and no `after-*` call ran before it. The plan is `timeout_k4.r3_calls`
unchanged. A control checks that it equals that sequence, with the pinned online
argument, the guest timeout of 120 s and the expected cancel exit -15.

The tooling was frozen in commit `ce913129c137` before the run. The run is
`experiments/exec-reliability/evidence/k193-r3-macos-01/d8bca6a5a87547899d23140a124e2ceb`.
Its outer records are in `k193-r3-macos-01-outer`. The tool digests before the probe
and after the check are equal. The recorded uncommitted diff holds only the outer
records that the runner wrote before it took the diff.

### Result

The checker `timeout_k193_check.py` applies checker revision 3 to the r3 calls alone.
It reports evidence `complete` and stall `not-reproduced`, and it exits 0. All 23 r3
calls ran, and none failed or stayed unreached.

| Call | Duration | Outcome |
|---|---|---|
| `r3-timeout` | 2.172 s | exit -1, timed out |
| `r3-observe-timeout` | 0.142 s | exit 0, timeout child PID 682 not listed |
| `r3-cancel` | 2.101 s | CLI interrupted, exit -15 |
| `r3-observe-cancel` | 0.139 s | exit 0, PID 682 not listed, cancel child PID 700 present |
| `r3-guest-cleanup` | 0.127 s | exit 0 |
| `r3-observe-after-cleanup` | 0.087 s | exit 0, PIDs 682 and 700 not listed |

The unchanged fixture runs `ps -p "$pid" ... || true`, so it hides a failure of `ps`.
A row that is not listed therefore does not prove that the process was absent. The
cancel child PID 700 was `sleep` in state `S` with parent PID 699 after the CLI
interruption. This is the third sample of that survival, after `macos-01` and k4 run 02
PID 812. The fixture's guest cleanup sends `kill` only to a PID that `ps` lists as
`sleep`. It ignores the result and records no outcome for each PID.

The later owned cleanup checked each PID with `kill -0` and printed `gone pid=682` and
`gone pid=700`. This shows that both processes were absent at that later time. It does
not show when each process exited. It also does not show which action ended PID 700.
Leaf `macos-exit-observation-instrumented-k195` must base each absence on a valid own
observation.

### Accounting

- The owned cleanup printed `gone pid=682`, `gone pid=700` and `cleanup confirmed`.
- The controller disposed of the clone `k193-r3-macos-d8bca6a5a87547899d23140a124e2ceb`.
  The ledger records `disposed` and the residual list is empty.
- The clone is absent from the Tart list and from the TestAnyware VM list after the
  run. The app PID was 1488 before and after the run, so the installed app was not
  stopped.

### What this adds to the candidates

One clean sample cannot exclude an intermittent cause. It shows that the complete r3
condition on a fresh clone does not reliably give the `macos-03` stall of 33 s. In k4
run 02, `r3-observe-timeout` also completed in 0.140 s, after the issue payloads.
So neither recent clone reproduced the original known-child stall.

The run gives no phase timestamps, so it does not separate the three regression
candidates. It does not show when `ps` or the shell exited, or when the agent saw the
exit. Leaf `macos-exit-observation-instrumented-k195` owns that question with one
instrumented clone.

## Exit observation with a traced agent (leaf macos-exit-observation-instrumented-k195)

Leaf `macos-exit-observation-instrumented-k195` asks one question. After an
intentional timeout, does a stalled guest command still run, or does the agent miss
an exit that already happened? The leaf adds a trace seam to the macOS agent. It then
runs the k4 run 02 workload once with that traced agent in one owned clone.

### Trace seam

The seam is `agents/macos/Sources/TestAnywareAgent/ExitTrace.swift`. It compiles only
with the flag `EXIT_TRACE`. It records each wait phase of `runShell` and the agent's
thread count. A kqueue thread records the kernel fork, exec and exit of the shell and
of each child. Each record carries a monotonic time in nanoseconds. The default release
agent holds no `ExitTrace` symbol, and its build is byte-identical to the build before
the change.

### Host controls

Four host controls force each suspected ordering with the traced agent. They are in
`experiments/exec-reliability/evidence/macos-exit-observation-k195/controls-01`. Each
control passed in each of three runs.

| Control | Ordering forced | Trace result |
|---|---|---|
| normal | none | exit at 2.3 ms, agent result at 2.4 ms |
| starved | 80 busy global-queue blocks | exit at 2.3 ms, timeout at 2006 ms, inner wait started at 2006 ms |
| late TERM | command runs past the deadline | timeout at 2006 ms, TERM succeeds, return at 2.12 s |
| ignored TERM | command ignores TERM | TERM wait times out at 5052 ms, then SIGKILL |

The starved control shows candidate 1 as a trace: the shell exits first, and the agent
still reports a timeout. The ignored TERM control adds the 3 s TERM wait. That wait
matches the difference between the 33 s and 30 s shapes. The controls show that the
checker can tell these orderings apart. They do not show which ordering the guest had.

### Route

The run used the isolated route of the root brief. On 2026-11-17 the owner authorised
one app handover. The leaf quit the installed app 2.8.0 and opened the checkout-built
`TestAnyware Dev.app` with build 0.0.1. The checkout-built CLI drove the run.

The runner built the traced agent from commit `9e096ac3cb82`. Its digest is
`76e76a484edd…6bcf`, and it holds 44 `ExitTrace` symbols. The probe uploaded the agent
into the clone and installed it with `sudo -n install`. It then restarted
`com.linkuistics.testanyware.agent` with `launchctl kickstart`.

The `verify-agent` call ran under agent PID 703. The agent before the install was PID
412. The first trace record is `trace-open` by PID 703, with the installed digest. So
the checker judged the route `traced`.

The new binary has a new code-signing identity, so it loses the clone's accessibility
grant. Both health calls after the install answered with `accessibility_status` denied
and CLI exit 4. Exec needs no grant, and each exec call answered.

### Result

The run is `experiments/exec-reliability/evidence/macos-exit-observation-k195/run-01/1cb859c380f64c50bc67d9afa75b992b`.
Its outer records are in `run-01-outer`. Checker `k195-1` reports route `traced`,
evidence `complete` and stall `not-reproduced`, and it exits 0. The trace holds 253
records. No record failed to parse, and the kqueue thread reported no error.

| Call | Host time | Deadline phase | TERM phase |
|---|---|---|---|
| `issue-sleep` | 2.149 s | command alive, `/bin/sleep` | TERM succeeds, exit 70 ms after deadline |
| `issue-child` | 2.270 s | command alive, shell and `/bin/sleep` | TERM succeeds, exit 127 ms after deadline |
| `after-03-table` | 0.140 s | observed | none |
| `after-09-table` | 0.146 s | observed | none |

The agent observed the exit of each of the nine other exec calls before its deadline.
No call other than the two intentional timeouts timed out. So the stall of k4 run 02
did not recur. In run 02, `after-09-table` timed out at 30.148 s. Here the same
command exited at 32.9 ms and the agent returned at 73.4 ms.

In these nine calls, the agent saw the exit from 25 ms to 65 ms after the kernel exit.
On the host, the same lag was less than 0.1 ms. The lag is far below the 30 s guest deadline.
The agent had at most 12 threads at any record. Candidate 1 needs 63 of the 64
constrained workers of the guest to be busy. So no starvation occurred in this run.

### Children and cleanup

In `issue-child`, the agent sent TERM to shell PID 735. The kernel recorded the exit
of PID 735 and of its child `sleep` PID 737 with status 15, 0.3 ms later. The later
SIGKILL to PID 737 returned errno 3, no such process. The trace shows the time of each
exit and the action that came before it. It does not show why TERM to the shell also
ended the child.

The later observation `issue-observe` did not list PID 737. That absence agrees with
the kernel exit record, which is the own observation that proves it. The owned cleanup
printed `gone pid=737` and `cleanup confirmed`.

### Accounting

- The controller disposed of the clone `k195-exit-macos-1cb859c380f64c50bc67d9afa75b992b`.
  The ledger records `disposed` and the residual list is empty.
- The clone is absent from the Tart list after the run.
- The tool digests before the probe and after the check are equal. The recorded
  uncommitted diff holds only the outer records that the runner wrote before it took
  the diff.
- The development app was PID 46272 before and after the run. After the disposal, the
  leaf quit it and reopened the installed app 2.8.0 as PID 53925.

### What this adds to the candidates

One clean sample cannot exclude an intermittent cause. The cause of the run 02 stall
stays unproved. The run removes no candidate, because the stall did not recur. It
does show that the traced route works in a clone, and that the checker reads each
ordering from a real guest trace. A later recurrence under this route would name the
ordering directly.

## Regression candidates

1. The k3 completed-then-timed-out mechanism. `waitForExit` waits on a second
   global-queue block, which can starve. The k3 detector
   `diagnostic:macos-completion-old-path` reproduces it in a probe. In this run it
   would show output that ends with the `ps` row, then a 30 s guest timeout. The
   receipt matches.
2. A `ps` exit delay on a process in state `U`. It would show the same receipt.
3. The `macos-03` shape of 33 s fits an agent that also missed the exit
   in its 3 s wait after TERM. Run 02 has the 30 s shape, which is consistent
   with a second wait that succeeded. No phase timestamp measures either wait.

The k195 trace seam now measures each wait. Its single guest run did not reproduce the
stall, so it does not yet separate these candidates.

## Relationship to ordinary completion

The relationship is unproved. Run 02 shows the post-timeout stall shape on a command
other than the observation. The k186 run of 22 ordinary requests on a clone without
intentional timeouts was clean, and its recheck is unknown. This run does not show
whether the intentional timeouts cause the later stall or only precede it.

## Commands

- `mise exec -- task diagnostic:macos-timeout-inspection OUT=<new dir>` runs the frozen
  plan on one owned clone.
- `mise exec -- task diagnostic:macos-timeout-inspection-check RUN=<run dir>` judges a
  retained run. It exits 0 only when the stall result is determined and the clone was
  disposed. On run 02 revision 3 exits 0 and reports `reproduced`.
- `mise exec -- task diagnostic:macos-timeout-inspection-tools` checks the tooling and
  runs 52 controls without a VM. A damaged input makes it exit non-zero.
- `mise exec -- task diagnostic:macos-timeout-inspection-r3 OUT=<new dir>` runs the
  complete original r3 condition alone on one owned clone.
- `mise exec -- task diagnostic:macos-timeout-inspection-r3-check RUN=<run dir>` judges
  a retained r3-only run with checker revision 3.
- `mise exec -- task diagnostic:macos-timeout-inspection-r3-tools` checks the r3-only
  tooling and runs its 13 controls with the 52 k4 controls, without a VM.
- `mise exec -- task diagnostic:macos-exit-observation-controls OUT=<new dir>` runs the
  four host controls with the traced agent, without a VM.
- `mise exec -- task diagnostic:macos-exit-observation OUT=<new dir>` runs the k4 run 02
  workload once with the traced agent on one owned clone. It needs the owner's app
  handover first.
- `mise exec -- task diagnostic:macos-exit-observation-check RUN=<run dir>` judges a
  retained traced run with checker `k195-1`.
- `mise exec -- task diagnostic:macos-exit-observation-tools` checks the k195 tooling and
  runs its 8 controls without a VM.
