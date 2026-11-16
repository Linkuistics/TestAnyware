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
