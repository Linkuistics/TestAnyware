# macOS synchronous completion timeout

Leaf k3 asks why one ordinary macOS command timed out after it had finished.
This report covers the first slice of k3. The slice preserves the retained
failure and records its identity. It also shows one ordering in the unchanged
agent code that gives the same receipt, and it delivers a detector for that
ordering. The slice did not run a guest. It makes no claim of a repair and no
claim that this ordering caused the retained failure.

## Retained failure

Issue #14 failure A comes from the run `macos-candidate-01` in the
Ravel-Strategies checkout. The run sent ten consecutive
`testanyware file exec --vm vmrel-k16-macos-candidate-01 --timeout 30 --json "printf ordinary-ok"`
calls. The run is not scored (`run.json`).

| Op | Host seconds | Host exit | Guest exit | `timed_out` | stdout |
| --- | --- | --- | --- | --- | --- |
| 01 | 0.091 | 0 | 0 | false | `ordinary-ok` |
| 02 | 0.088 | 0 | 0 | false | `ordinary-ok` |
| 03 | 0.082 | 0 | 0 | false | `ordinary-ok` |
| 04 | 0.103 | 0 | 0 | false | `ordinary-ok` |
| 05 | 0.090 | 0 | 0 | false | `ordinary-ok` |
| 06 | 0.089 | 0 | 0 | false | `ordinary-ok` |
| 07 | 0.023 | 0 | 0 | false | `ordinary-ok` |
| 08 | 30.058 | 0 | -1 | true | `ordinary-ok` |
| 09 | 0.028 | 0 | 0 | false | `ordinary-ok` |
| 10 | 0.101 | 0 | 0 | false | `ordinary-ok` |

Operation 8 started 0.040 s after operation 7 had started. Operation 7 took
0.023 s, so the calls did not overlap on the host. The receipt of operation 8
has the complete stdout and the stderr `Process timed out after 30s`. So the
child ran and wrote its output, but the agent did not see its exit within
30 s.

The agent reported the timeout 0.058 s after the 30 s deadline, measured on
the host. After a timeout, `runShell` lists descendants, sends SIGTERM and
waits up to 3 s for the exit again. That second wait succeeded promptly. The
agent therefore saw the exit right after the deadline, but not during the
30 s before it.

### Preservation

`evidence/macos-completion-k3/retained.py` hashes the 319 files of the
retained run and four setup and upstream files into
`retained-manifest.json`. It copies the 32 receipts that this report cites to
`evidence/macos-completion-k3/retained/`. The original files stay in the
Ravel-Strategies checkout. After all k3 runs,
`task diagnostic:macos-completion-retained` reported
`compare: 319 retained files, unchanged=True`.

### Identity

| Subject | Identity | Source |
| --- | --- | --- |
| Host | macOS 27.0.1 (26A434), Darwin 27.0.0, arm64 | `setup/host.txt` |
| Host CLI | sha256 `2298b1bd…0720` | `setup/tools-before.txt` |
| App executable | sha256 `b7a51852…60bb` | `setup/tools-before.txt` |
| Guest agent | `/usr/local/bin/testanyware-agent`, 20551760 bytes, 18 October 18:29, sha256 `1282f4cf53fe2ede6c7e94a2687888b5fa4ada3e2a920d227db2715679b28a84` | `macos-candidate-01/inventory.json` |

**Provenance gap.** No local build has the hash `1282f4cf…8a84`. The
Homebrew 2.8.0 agent has sha256 `22dd56ce…b9f5`. The source revision of the
old agent is therefore not established.

`ProcessRunner.swift` and
`handleExec` have not changed since commit `868ca80308` (19 April 2026). If the
old agent came from this repository after that date, it carries the same two
functions. That is an inference, not an observation. Every k3 build is a new
condition.

## Mechanism under test

`ProcessRunner.waitForExit` takes one worker of the global dispatch queue. On
that worker it starts a second global block, which calls
`process.waitUntilExit()` and signals a semaphore. The first worker then waits
on the semaphore with the timeout.

The first worker holds its thread while it waits. The second block needs
another thread. If the pool has exactly one free thread, the first block takes
it and the second block waits in the queue. The semaphore then times out even
though the child exited at once. When the first block returns, its thread
becomes free and the second block runs.

### Detector

`evidence/macos-completion-k3/probe/main.swift` is compiled together with the
unchanged `ProcessRunner.swift`. It calls the real `ProcessRunner.runShell`
with `printf ordinary-ok`. Before the call, it occupies the global queue in one
of three ways.

| Condition | Occupation |
| --- | --- |
| `unsaturated` | none |
| `free-2` | block every worker that GCD gives, then free two |
| `free-1` | block every worker that GCD gives, then free one |

The probe calibrates the pool. It adds blockers until a new blocker does not
start within 500 ms. A blocker that starts after that point returns at once.
The freed workers therefore stay free. The probe records `saturated_at`, and it fails
the trial when calibration does not finish. It releases all blockers before
it exits.

`evidence/macos-completion-k3/old_path.py` writes the plan, bounds and subject
hashes before the build. It runs five rounds of the three conditions with a
3 s guest timeout and a 20 s bound for each trial. A trial is
`completed-then-timed-out` when stdout is `ordinary-ok`, `timed_out` is true
and the exit is -1. A trial is `prompt` when the exit is 0 within 1 s. The
verdict is `defect` when every `free-1` trial completed and then timed out and
every control was prompt.

The detector control `--variant single-hop` builds a copy of
`ProcessRunner.swift` in which the first worker polls `process.isRunning`
itself. The copy has no second block. It is a control for the detector, not a
proposed repair.

### Results

Both runs used swiftc 6.4.2 on the k19 host, with 16 active CPUs and
`kern.wq_max_constrained_threads` 80. Every calibration saturated at 80
workers, which equals that kernel limit.

| Run | Variant | `unsaturated` | `free-2` | `free-1` | Verdict |
| --- | --- | --- | --- | --- | --- |
| `old-path-01` | old | 5 prompt | 5 prompt | 5 completed-then-timed-out | `defect` |
| `single-hop-01` | single-hop | 5 prompt | 5 prompt | 5 prompt | `clean` |

Each `free-1` receipt of the old path has `stdout: "ordinary-ok"`,
`exit_code: -1`, `timed_out: true` and `stderr: "Process timed out after 3s"`.
That is the receipt of operation 8 with a 3 s timeout. The old path took
about 6.05 s inside the probe. The second wait after SIGTERM also starved
because one worker was still free.

Subject hashes before and after both runs are equal. The old build has
source sha256 `f9401669…6340` and binary sha256 `dcbf52fb…ba45`.

## Causes

| Candidate | Evidence for | Evidence against | State |
| --- | --- | --- | --- |
| Global-queue starvation delays the second block | Same receipt shape, reproduced 5 of 5 with the real code. The control removes it. | The retained recovery wait was prompt. A constant one-worker probe makes it starve. No one observed a guest starvation source. | Possible, not established |
| `waitUntilExit` missed the exit in the second block | None | The retained recovery wait saw the exit within 0.058 s. | Open, not tested |
| Capture-pipe EOF | None | `runShell` captures to files, not pipes, and stdout was complete. | Excluded for this path |
| Client deadline or app relay | None | The receipt came from the agent with `timed_out: true`, and the host exit was 0 within the 50 s bound. | Excluded as the source of the timeout |
| Guest load or scheduler stall | None | Unobserved | Unknown |

The retained run gives no observation of the guest thread pool, the second
block or the child exit time. Those phases stay unknown.

## Design implications

A bounded wait must not depend on a second block in the same pool as the
waiter. Any repair belongs to a later leaf, and this slice adds no production
wait or cancel semantics. A repair must turn `diagnostic:macos-completion-old-path`
with `EXPECT=clean` green on the changed code.

## Regression invocation

```sh
mise exec -- task diagnostic:macos-completion-retained
mise exec -- task diagnostic:macos-completion-old-path OUT=<new directory>
mise exec -- task diagnostic:macos-completion-old-path OUT=<new directory> VARIANT=single-hop EXPECT=clean
mise exec -- task diagnostic:macos-completion-tools
```

The second command reports the known defect. It exits 0 when the verdict is
`defect`, so it stays outside the passing gate. Today `EXPECT=clean` on the old
variant exits 1.

## Owned-resource audit

The slice started no VM, app, clone or guest command. Each probe process
started one `bash` child, and `ProcessRunner` waited for each child. Each probe
released its blockers before it exited, and each trial process exited within
its bound. No trial reached the 20 s bound. The slice ran no process cleanup
by name.

## Remaining k3 work

The slice leaves these items of k3 to later leaves:

- A fresh unchanged macOS clone with ten exact serial operations. The guest
  timeout is 30 s and the host bound is 50 s. A predeclared concurrent control
  runs on the same build.
- Guest observation of the dispatch pool, the child exit and the second block,
  at most one instrumented clone.
- Owned process fixtures in `ProcessRunnerTests.swift`, which replace the
  `pkill -f 'sleep 30'` cleanup before `agent:macos:test` runs.
