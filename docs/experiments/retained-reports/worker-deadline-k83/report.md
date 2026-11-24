> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Fixed worker shutdown authority

The interim per-query worker now keeps its original shutdown deadline on its
owner. Both signal and exact-child reap entry points sample that deadline and
return at or after it. The final cleanup call and stale callbacks therefore
cannot renew the allowance. An unreaped worker remains strongly owned and is
reported as a residual obligation.

Before the deadline, readiness still selects PID-only or process-group signals,
and reap still uses the exact child with `WNOHANG`. Helper-local late-creation
cleanup is unchanged. The receipt's existing `shutdown_deadline_monotonic` field
now comes from the owner rather than a duplicate receipt assignment.

## Retained verification

| Condition | Result | Evidence |
| --- | --- | --- |
| Original runtime, initial six controls | Six expected failures on late OS calls | [RED 1](red-1/verdict.json), [JUnit](red-1/pytest.xml) |
| Original runtime, expanded matrix | All eight cases fail on late OS calls | [RED 2](red-2/verdict.json), [JUnit](red-2/pytest.xml) |
| Changed runtime, identical expanded controls | The same eight cases pass | [GREEN](green-1/verdict.json), [JUnit](green-1/pytest.xml) |
| Full diagnostic suite | 432 cases pass; Ruff lint/format and strict pyright exit zero | [Full verdict](full-1/verdict.json), [JUnit](full-1/pytest.xml) |
| Evidence reconciliation | Case identities, fixture audits and hashes match; 89,102 prior file entries unchanged | [Audit](audit-1/verdict.json) |

The matrix uses real owned children, with and without a readiness publication.
Each child resets inherited state and waits for a real signal; no helper is
created. Signal wrappers record and forward actual `kill`/`killpg` calls.
The parent wait adapter deliberately defers observation of an exited child.
This is simulated delayed observation, not kernel unreapability.

For each signal scope, the controls cover the cleanup loop's final reap, stale
TERM, stale KILL and exact equality with the cutoff. The equality control fixes
the clock at the original deadline only while invoking stale entry points.
Every GREEN/full audit records no OS call at or after that deadline, a failed
capture with residual ownership, and a separate exact-child reap outside the
runtime adapter. Actual termination statuses are retained. RED final-reap
cases separately identify status consumed by the late runtime call.

These fixtures intentionally exhaust their 300 ms allowance. Their captures are
failures, and elapsed overruns remain failures; passing tests establish refusal
and retained ownership. A clock sample cannot prevent later process preemption
or an OS call from overrunning. No hard real-time guarantee follows.

Every checker completed without timeout and was reaped. The full pytest run
took 54.29 seconds inside its 60 second budget. Condition declarations freeze
the interpreter, `/bin/ps`, `/usr/bin/true`, sources, manifests, Taskfile, runner
and prior inventory before execution and compare them afterward. The full
self-check additionally freezes the suite's `/bin/sh`, `/bin/sleep` and
`/usr/bin/false` subjects. Frozen GREEN inputs precede this metadata-only checker
addition; the auditor checks those exact four added executable paths as the
only difference from live `self_check.py`.

The auditor reconciles individual JUnit identities, requires the expected RED
failure messages and checks every raw artifact hash. It also rehashes the
original prior-file inventory. That inventory hashes file bytes, including
symlink-file targets; it excludes symlink text and directory links. A separate
jj path comparison found every changed evidence path inside this new directory.
The audit is producer verification; a separate review supplies the adversarial
assessment.

## Reproduction

Choose a fresh output directory for every measurement:

```sh
mise exec -- task diagnostic:worker-deadline-check INPUTS=experiments/exec-reliability/evidence/worker-deadline-k83/red2-inputs OUT=/tmp/worker-deadline-red-new
mise exec -- task diagnostic:worker-deadline-check INPUTS=experiments/exec-reliability/evidence/worker-deadline-k83/green-inputs OUT=/tmp/worker-deadline-green-new
mise exec -- task diagnostic:self-check OUT=/tmp/worker-deadline-full-new
```

RED intentionally exits nonzero. `diagnostic:resource-audit` ran with `AUDITOR`
pointing to `audit.py` here. Its fixed `audit-1` destination refuses replacement;
preserve it and use a separate evidence copy for a repeat audit.

## Remaining work

`complete-worker-scheduler-k85` retains the entire persistent-worker/shared-
scheduler aggregate charter, its outstanding controls and final aggregate
review. This repair establishes one boundary in the interim per-query transport.
`bounded-inspection-k28` and `diagnostic-evidence-k17` remain open. No guests,
installed app changes or privileged operations were used. Actual privileged
UID/KILL controls and ten-clone acceptance remain unavailable (0/10).
