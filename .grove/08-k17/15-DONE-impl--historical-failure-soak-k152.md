# historical-failure-soak-k152

## Goal

Find a cause for each of the six k17 failures that have none. Where no cause
appears, record a bounded soak in which they do not recur. Add the timestamps that the retained
receipts lack, so that the evidence can separate the possible causes.

## Context

The human chose this soak over accepting the six cases as visible limits. The
cases are in
`experiments/exec-reliability/evidence/failure-controls-k151/verdicts.json`:
`interrupted-live-group-k145`, `exited-leader-grace-k132`, `prebuffer-k106`,
`helper-capture-deadline`, `empty-pid-k85` and `checker-ceiling-k77`. The k151
`report.md` gives the observations and the controls that ran.

The k145 and k132 peers printed the child PID, so readiness came before KILL.
No receipt records the readiness time or the leader exit time. Each of the five
bound cases ran out a bound, and the runtime failed closed. No control
reproduces those stalls.

## Done when

- The instrumentation records, for each soak run, the times of child readiness,
  TERM, KILL, leader exit and helper start and end. It also records the system
  load. A frozen control shows that each field is present and ordered.
- One soak plan states the workloads, the run count, the load conditions
  and the wall-clock budget before it starts. The soak runs that plan once in a
  fresh directory, with an itemised resource audit.
- Each recurrence of a historical shape gets a mechanism from the recorded
  timestamps, or a precise statement of what the timestamps rule out.
- If no shape recurs, the result states the run count and the conditions, and
  makes no claim of cause. The verdicts then record each case as "did not recur
  in N runs", not as explained.
- A repair follows only a demonstrated mechanism. It has a red/green control
  against frozen inputs.
- The verdicts, the Taskfile, the handoff
  `docs/experiments/exec-reliability-diagnostic-foundation.md` and the k17
  whole-contract assessment reflect the result.

## Notes

The soak runs once to its stated plan. Do not extend it or rerun it to get a
cleaner sample. Keep every failed run and its timing. Do not raise a bound to
make a run pass. k18 owns stalled storage. No live guest, app or ten-clone
acceptance claim follows.

## Decisions (running log)

- checker-ceiling-k77 has a mechanism from retained timestamps. No soak is
  needed for it. k77-green-02 ran `test_worker_capture.py` sha256 `450e98b7…`.
  Its late-created fixtures used a 0.6-second deadline. k77-green-03 ran
  `e7e808bf…` with a 0.4-second deadline (commit `c62a9e9`). Witness file names
  carry `time.time_ns()` at test start. Up to 45 seconds the two runs agree
  within 1 percent. After that, 26 green-02 captures took 14.54 seconds, and 27
  green-03 captures took 9.70 seconds. green-02 reached the final test at 59.54
  seconds after pytest start, 5.02 seconds behind green-03. The 60-second cap
  then stopped it. The cause is the input runtime, not a stall.
- prebuffer-k106 has no mechanism from retained timestamps. The four k106 runs
  (`full-final` to `full-final-4`) used the same runtime and test hashes. Only
  `full-final-2` failed in the late-created block. From 5.16 to 8.75 seconds
  into that block, six workers outlived the 12.5-millisecond TERM grace
  (`reserve / 4`, `worker_capture.py:1356`). The runtime then sent KILL. In the
  same window, the gaps between tests rose from about 3 milliseconds to 60–160
  milliseconds. The three passing runs show no KILL and no such gaps. Four of
  the six KILLed workers are the `after_term` KeyError cases. Both prebuffer
  witnesses hold the identity frame but not `pipe_before_term`, 1.1 seconds
  after the window ended. No receipt survives, because the assertion fires
  before `retain`. The record cannot separate a late helper exec, a late reap or
  a late terminal write. The soak must stamp helper start and end inside this
  fixture.
- empty-pid-k85 receipts sharpen the k151 statement. The record is
  `termination-cutoff-k85/full-1/runs`. In three of the four failures, TERM
  came at 0.300 to 0.301 seconds. The leader exited 6 to 9 ms later with
  return code -15. That code is the default SIGTERM action. So the peer had
  not yet reached `signal.signal` at `peer.py:39`, and it started no
  child. Run `bffea7a4` confirms this. Its group snapshot holds only the
  zombie leader. The fourth failure, run `93ebab49`, is different. Its TERM
  came at the 5-second budget, and the leader did not exit within the
  151 ms grace. KILL ended it with return code -9. The retained record cannot
  separate two causes. The peer startup and `Popen` took more than 1 second,
  or something blocked the leader where TERM could not act. Three of the four group
  snapshots also show the ps helper past its capture deadline. All four runs
  fall in one 6.7-second wall-clock window. The cause of the slow interpreter
  start is unknown. The soak must stamp peer handler installation, child spawn
  and child readiness.
- Instrumentation. With `PEER_STAMPS` set, `peer.py` appends monotonic
  events to one file per process. The events are start, handler installation,
  child spawn, child readiness, PID print and loop exits. Without the variable
  the peer behaves as before. The prebuffer fixture now stamps the identity
  write and the prebuffer branch. A separate parent file stamps the parent
  wait, so each file keeps one writer. The receipts already hold TERM, KILL,
  leader exit and ps helper start and end. `timeline.py` joins these sources
  into one record per workload. `soak.py` adds the one-minute load average
  before and after each run and samples it each second.
- Soak plan, frozen before the first run in
  `evidence/historical-soak-k152/plan.json` (sha256 prefix `13558d5168b3d2a0`).
  Workload `targeted` runs the four test functions of the open cases, which
  are 7 tests. Workload `full` runs the whole suite of the frozen inputs. Runs
  alternate idle and loaded, idle first. Loaded runs add 32 busy Python
  processes, two per logical CPU. A trial with 16 processes slowed one targeted
  run by only 5 percent, so the plan uses 32. The soak is 40 targeted runs,
  then 6 full runs. The wall-clock budget is 2400 seconds. The tool records
  each run it does not start inside the budget. It never retries a run.
- checker-ceiling-k77 is outside the soak claim. It has a mechanism, and the
  full workload only exercises it again.
- Result. The frozen control passed, and it detected each of 87 damaged
  copies. The soak completed 46 of 46 runs in 811.5 seconds. Its subjects did
  not change. No historical shape recurred. Five cases now say "did not
  recur in 46 runs", with exposure counts, and claim no cause.
- Run 42 failed `test_worker_shutdown_deadline_closes_owner_operations[stale-kill-group]`
  and, as a cascade, `test_review_worker_rejects_fork_before_creation`. A
  host-wide stall of about 160 milliseconds crossed the 0.3-second fixture
  deadline. The load sampler, in another process, woke 161 milliseconds late
  in the same window. The runtime correctly sent no signal. The fixture has no
  fallback cleanup, so it leaked child PID 4100 and its owner record.
- This leaf does not repair the fixture. The failure is new, not a historical
  shape, and its repair needs its own red/green control. Leaf
  `worker-deadline-cleanup` owns it. The soak tool also stays frozen. Its
  audit missed worker forks of pytest, so a manual scan found PID 4100. A
  group KILL removed it, and `resource-audit.json` records each step.
- The soak result is bounded. Whether it is enough for the k17 whole contract
  is a human decision. The verdicts keep that contract not met.
