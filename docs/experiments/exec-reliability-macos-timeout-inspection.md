# macOS post-timeout inspection stall (leaf macos-timeout-inspection-k4)

This report accounts for issue #14 failure A on macOS. Failure A is the stall of a
30 s guest inspection that runs immediately after an intentional 2 s timeout. The
report keeps each old failure apart from later samples and from the ordinary-command
timeout of the k3 report.

## Result

One fresh unchanged clone reproduced a post-timeout stall. The guest command was a
process listing, not the known-child `ps -p` observation. The listing printed its
whole table, including its own `ps` row, and the agent then reported a guest timeout
at 30 s. The same command ran in 0.150 s three operations earlier on the same clone.

The evidence is incomplete. The complete original r3 fixture condition stopped at its
cancel case because of a plan defect. The cause of the stall is not proved. The
shape is consistent with the k3 completed-then-timed-out mechanism, but no evidence
in this run shows when the guest shell exited.

## Old failures, kept separate

The original failed inspections stay in the retained VM-fit workspace
`Ravel-Strategies.gh-issue-1/experiments/tutorial-vm-fit/evidence/`. This leaf reads
them and does not copy them.

| run | fixture | `observe-timeout` | receipt |
|---|---|---|---|
| `macos-01/clone-1` | r1 | 0.111 s, exit 0 | `timeout pid=696`, no `ps` row: the child was gone |
| `macos-02/clone-1` | r2 | guest timeout at 30 s | `timeout pid=709`, then `Process timed out after 30s` |
| `macos-03/clone-1` | r3 | 33.067 s, guest timeout | `timeout pid=666`, then `Process timed out after 30s` |

The `macos-01` receipt shows that `ps -p` prints nothing when the child is gone. So
the `macos-02` and `macos-03` stdout is also complete output. Those receipts do not
show whether `ps` blocked or whether the command completed and the agent did not
observe the exit. The `macos-03` elapsed time of 33 s is the 30 s deadline plus the
agent's 3 s second wait. That second wait also timed out, so the agent did not
observe the shell exit within 3 s after TERM.

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

The plan `experiments/exec-reliability/timeout_k4.py` freezes one clone of
`testanyware-golden-macos-27` and 36 calls in this order:

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
| `issue-observe` | 4.524 s | 0.156 s | `child pid=630`, no `ps` row, exit 1 |
| `after-01` to `after-08` | 4.68 s | 0.14 s to 0.18 s each | normal |
| `after-03-table` | | 0.150 s | 459 rows |
| `after-09-table` | 5.808 s | 30.148 s | exit -1, timed out, 460 rows, last row `695 421 … ps -axo …` |
| `after-10-health` | 35.957 s | 0.081 s | healthy |
| `r3-timeout` | 38.150 s | 2.209 s | exit -1, timed out, overhead 0.209 s |
| `r3-observe-timeout` | 40.359 s | 0.140 s | normal, child gone |
| `r3-cancel` | 40.500 s | 2.108 s | interrupted, CLI exit -15 |

The three intentional timeouts had host overheads of 0.209 s to 0.281 s. Each
overhead includes the host transport, so each one is an upper bound on the agent's
work after its deadline. No overhead exceeded the 4 s sum of the TERM grace and the
kill/drain/reap bound, so the run records no grace or reap violation.

The agent killed the issue child: `issue-observe` found no process for PID 630. The
r3 cancel child, PID 812, survived the CLI interruption, as in `macos-01`. The
resource cleanup terminated it by its pid file, only after it checked that the
command was `sleep 120`.

### The stalled call

`after-09-table` ran `ps -axo pid,ppid,pgid,stat,lstart,etime,command`. Its stdout
holds 460 rows. `ps` sorts by PID, and its own row (PID 695, parent 421, the agent)
is the last row. So `ps` wrote its complete table. The stderr holds only the agent's
note `Process timed out after 30s`. The elapsed time is 30.148 s, so the agent's
second 3 s wait after TERM succeeded at once.

What this shows about the waiting operation:

- It did not wait on an earlier request. The previous request ended 0.15 s before,
  and the agent answered each of the eight earlier after-timeout calls in under 0.2 s.
- It did not wait on host transport. The host received a guest receipt that states a
  guest timeout, and the next health call took 0.081 s.
- It did not wait on descendant enumeration as the first cause. The agent enumerates
  descendants only after its 30 s deadline expires.
- It waited on its own child or on the agent's exit observation. The output is
  complete. Either `ps` or the shell did not exit. Or both exited, and the agent did
  not observe the exit within 30 s. This run cannot distinguish the two cases.

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

## Accounting

- The ledger shows the clone `k4-macos-6de306c9…6979` disposed in 1.215 s, and the
  controller named no residual process. The VM inventory after the run holds no clone.
- The resource cleanup acted only on the three known pid files. It found PIDs 630 and
  799 gone, and terminated PID 812.
- No broad process-name cleanup ran. The host app was not stopped.

## Plan defect and unfinished conditions

The plan gave `r3-cancel` the default expected exit 0. The interrupted CLI exits -15,
so the controller marked the call failed and stopped the plan. These r3 calls did not
run: `r3-observe-cancel`, the two partial downloads, `r3-missing-artifact`,
`r3-guest-cleanup` and `r3-observe-after-cleanup`. The plan now expects exit -15, and
a control checks that value.

These conditions of the leaf remain open:

- The complete original r3 fixture condition, as a separate condition.
- An instrumented clone that records when the shell and `ps` exit and when the agent
  observes the exit. Only such a run can separate a blocked `ps` from a lost exit.

## Regression candidates

1. The k3 completed-then-timed-out mechanism. `waitForExit` waits on a second
   global-queue block, which can starve. The k3 detector
   `diagnostic:macos-completion-old-path` reproduces it in a probe. In this run it
   would show as complete output with a 30 s guest timeout, which matches.
2. A `ps` exit delay on a process in state `U`. It would show the same receipt.
3. The `macos-03` shape, 33 s, means the agent also missed the exit during its 3 s
   wait after TERM. Run 02 has the 30 s shape, so its second wait succeeded.

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
  disposed. On run 02 it exits 0 and reports `reproduced`.
- `mise exec -- task diagnostic:macos-timeout-inspection-tools` checks the tooling and
  runs 19 controls without a VM. A damaged input makes it exit non-zero.
