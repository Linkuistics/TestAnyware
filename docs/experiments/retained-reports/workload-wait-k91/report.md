> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Active workload exit-wait deadlines (k91)

`controller._wait` used to call the exit observer before checking the absolute deadline and accept a true result without checking its return time. A delayed real NOTE_EXIT result could report success after the workload/interruption budget, bypassing the required termination path.

The loop now samples admission before every observer call, including after a poll sleep, and checks the return time before accepting either result. Equality is expired. A timely false result sleeps for at most five milliseconds or the remaining budget. OSError continues to propagate to `_call`'s existing observation-error handling. This repair preserves the existing bool interface and the distinct inclusive first exact-child reap boundary.

## Controls

The twelve `workload_wait` cases in `test_controller.py` cover entry at/past the deadline (2), true/false results at/past it (4), wakeup at/past it (2), immediate/polled timely exit (2), and real delayed exit through resource acquisition, workload, unreached subsequent work, cleanup, disposal and absence inspection (2). The real cases use a wall-clock hold around a real NOTE_EXIT query, with no synthetic exit status or signal errno. They retain the observed entry/deadline/return times, every controller receipt, and independent `/bin/ps` identity observations of all workload leaders, inspection helpers and workers after controller completion.

Ordinary deadline expiry attempts group TERM and mandatory group KILL even when the leader already exited. Interruption uses PID-only TERM and suppresses KILL after exit observation; it never gains group authority. Darwin can return EPERM for group TERM against the zombie. That denied TERM stays sticky, independently of the final KILL classification. The local zombie behavior does not certify the unavailable privileged UID fixture.

## Measurement records

Every condition has its own immutable directory with command/bound declarations, before/after SHA-256 subject inventories, pytest stdout/stderr and per-case JUnit, artifact hashes, checker PID/PGID/reap receipts, and raw real-run evidence. The runner freezes all input root Python modules, pyproject.toml, uv.lock, frozen and active Taskfile.yml, itself, the actual Python executable, /bin/ps, /bin/sh, /bin/sleep, /usr/bin/true and /usr/bin/false. Checks have an outer 20-second focused/120-second full allowance, with TERM then KILL/reap allowances of two seconds each on timeout.

- `inputs-old-2` uses the unchanged pre-repair controller and final controls. `red-2` records 8 failed and 4 passed; both real runs incorrectly succeed and reach subsequent work.
- `inputs-final-2` freezes the final code and controls. `green-2` records all 12 passing.
- `inputs-mutant-admission` removes the loop admission bound. `inputs-mutant-result` accepts true before checking its return time. `inputs-mutant-sleep` checks admission only once before the loop. Each retains the final controls and all other inputs.

The original `red`/`inputs-old` sample used the first test draft. The original `green`/`inputs-final` sample records 10 passed/2 failed because that draft incorrectly required group KILL for interruption and successful termination despite a denied zombie group TERM. These samples remain historical and are not final controls. `full` records 480 passed/1 failed: relocating the suite broke a pre-existing self-check test's repository-relative Taskfile lookup. This is a frozen-layout failure, not a passing measurement. `full-2` runs the actual repository source, whose measured source hashes must equal `inputs-final-2` before it can certify that frozen version.

## Final outcomes

| Condition | Passed | Failed | Meaning |
| --- | ---: | ---: | --- |
| red-2 | 4 | 8 | Old controller exposes each deadline defect, including both real runs |
| green-2 | 12 | 0 | All final focused controls pass |
| full-2 | 481 | 0 | Actual repository suite; Ruff check/format and strict Pyright also pass |
| mutant-admission | 8 | 4 | Both entry and post-sleep boundary pairs detect removed admission |
| mutant-result | 8 | 4 | Late true boundary pair and both real runs detect premature success |
| mutant-sleep | 10 | 2 | Post-sleep boundary pair detects one-time-only admission |

`verification.json` checks exact case identities and expected failures, no skipped/error cases, no checker timeout or unreaped checker, equality of every condition's before/after inventory, current bytes for every declared subject and recorded artifact, all twelve root input files matching the frozen final copy, and independent absence of five workload leaders, five helpers and five workers in each of both real final focused runs and both corresponding full-suite runs. The runner's own Ruff, formatting and strict Pyright checks pass; their output is retained in `tools.stdout`/`tools.stderr`. The additional explicit `--project experiments/exec-reliability` Pyright invocation selects the strict configuration for the runner; its exact argv and output hashes are in `verification.json` and `runner-strict.stdout`/`runner-strict.stderr`. Raw failing condition verdicts remain false: matching expected mutation/RED failure is not a reinterpretation as runtime success.

Prior evidence preservation uses the jj content tree: baseline revision `3ab5a2cebccfe83f5367628e319d6be1c483d80b` contains 94,996 tracked evidence paths; the sorted path inventory SHA-256 is `578482369a2e1c3deb896fde919f638d62423785be17cd8ef8aefb33bd907d4b`. `jj diff --summary` has only additions beneath this new k91 evidence directory and no change/removal to any prior evidence. This verifies tracked history preservation without copying the prior corpus into another archive. All nine samples' source/tool/runner and artifact hashes were rechecked after the last measurement; no sample inventory moved. All 3,325 required frozen inputs and condition artifacts are tracked. Twelve intentional output/frame controls exceed jj's default new-file limit; a one-command `snapshot.max-new-file-size=25000000` override captured them without changing persistent configuration.

## Scope

This is an interim polling-loop correctness repair. It cannot preempt a synchronous exit observer, still has a private polling loop, and does not implement the persistent worker or earliest-deadline scheduler. The full original aggregate charter, all remaining controls and the final bounded-inspection review are preserved in complete-worker-scheduler-k94. k28, k17 and root remain open. Ten-clone acceptance stays 0/10 unavailable; no guests, installed app mutations, privilege or unrelated-process cleanup occurred. Prior evidence is preserved.

## Reproduction

Run `mise exec -- task diagnostic:workload-wait-check INPUTS=experiments/exec-reliability/evidence/workload-wait-k91/inputs-final-2 OUT=<fresh-directory>` for focused controls. Set INPUTS to the actual `experiments/exec-reliability` and MODE=full for the repository-layout full suite. Use the old or mutant input directory for the corresponding expected failing condition. `mise exec -- task diagnostic:workload-wait-tools` checks the retained runner.
