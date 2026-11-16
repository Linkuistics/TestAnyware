# k151 integration of the k150 review

This directory applies the three k150 findings to the k149 failure classification.
The k149 evidence stays unchanged. `verdicts.json` here replaces the k149 verdicts.
It records the SHA-256 of the k149 file and marks each changed case with `k151_change`.

## Result

The k17 whole contract is not met. Six cases have no demonstrated cause:

| Case | k151 classification |
|---|---|
| `interrupted-live-group-k145` | `defect_demonstrated_attribution_unresolved` |
| `exited-leader-grace-k132` | `bound_exhausted_cause_unknown` |
| `prebuffer-k106` | `bound_exhausted_cause_unknown` |
| `helper-capture-deadline` | `bound_exhausted_cause_unknown` |
| `empty-pid-k85` | `bound_exhausted_cause_unknown` |
| `checker-ceiling-k77` | `bound_exhausted_cause_unknown` |

The other eleven cases keep their k149 classification. The `exiting-state-k110`
mechanism text now matches the runtime comment in `controller.py`. Kqueue reports
ESRCH once `proc_find` fails, not from the start of exit.

## R1: historical attribution of the interrupted group

The k145 peer prints the child PID only after its readiness loop ends.
k145 run `179b4702` printed PID 34536, and k132 run `fc704d02` printed PID 16638.
So in both runs, readiness came before KILL. The k151 never-ready control gives
return code −9, but the peer prints no PID. That condition is therefore not the
historical cause.

Readiness that came late, followed by a slow exit, is possible.
No retained receipt shows the readiness time, so this cause is unproven.

The demonstrated defect remains real. The k145 peer ignores TERM until readiness.
Commit d5251d7 (k143) repairs it, and the final peer returns 0 in every condition.

## R2: child absence needs an identity and a clean ps result

The k149 tool counted a missing child identity as absence. It also counted any
failed ps query as absence. The k151 `scope_readiness.py` takes the identity from
peer stdout, else from the `.spawned` declaration of the final peer. A child is
"absent" only when `/bin/ps -p PID` exits 1 with empty stdout and stderr. Every
other result is "unknown", with a reason. Each probe keeps its return code,
stdout and stderr.

`absence_controls.py` feeds real ps results to both predicates:

| Case | Expected | k149 | k151 |
|---|---|---|---|
| No identity | unknown | absent (fail) | unknown |
| `ps: process id too large` | unknown | absent (fail) | unknown |
| `ps: Invalid process id: -5` | unknown | absent (fail) | unknown |
| Reaped child | absent | absent | absent |
| Live child | present | present | present |

A supplementary scan of the full process table looks for the unique case path in
argv. A positive control finds a holder process by that path, and the scan is
empty after the control reaps the holder. The scan never sets "absent".

Fresh reruns at load average 5.6, with three repeats each:

| Inputs | Offsets −0.2, +0.05, +0.13 s | Never ready |
|---|---|---|
| k145 peer | return code 0, child absent | return code −9, PID KILL, child unknown |
| Final peer | return code 0, child absent | return code 0, child absent |

In the three never-ready k145 runs, the child crashes when it writes its ready
file. `live-exit.stderr` keeps the traceback. The tool has no identity, so the
child is unknown. The case-path scans find no process in any run.

## R3: missed bounds are not shown to be unavoidable

No control reproduces a historical stall or identifies its cause in these five
cases. Each case keeps its observations and adds a `demonstrated` field where a
control shows part of the mechanism:

- `empty-pid-k85`: a startup stall reproduces the exact error. The fixture
  depends on startup within 0.3 s. The cause of the historical stall is unknown.
- `exited-leader-grace-k132`: controls measure exit 33–50 ms after TERM. None
  reproduces the exits at 158.3 ms and 167.8 ms.
- `helper-capture-deadline`: the runtime killed and reaped each late ps helper
  and failed closed. The control's maximum ps latency is 62.1 ms in 340 queries.
- `prebuffer-k106` and `checker-ceiling-k77`: no control ran.

## Reproduction

```sh
task diagnostic:absence-controls OUT=<new directory>
task diagnostic:scope-readiness-k151 INPUTS=evidence/caller-cleanup-k145/candidate-v2 OUT=<new directory>
task diagnostic:scope-readiness-k151 INPUTS=. OUT=<new directory>
task diagnostic:failure-tools
```
