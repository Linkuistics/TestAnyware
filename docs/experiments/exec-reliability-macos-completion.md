# macOS synchronous completion timeout

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

Leaf k3 asks why one ordinary macOS command timed out after it had finished.
This report covers the first slice of k3. The slice preserves the retained
failure and records its identity. It also shows one ordering in the unchanged
agent code that gives the same receipt, and it delivers a detector for that
ordering. The slice did not run a guest. It makes no claim of a repair and no
claim that this ordering caused the retained failure.

Leaf k186 later ran the exact workload on one fresh unchanged guest clone (see
Live clone run). All 22 requests completed. That clean run is not a repair, and
it neither establishes nor excludes a cause.

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
the host. In the current source, after a timeout, `runShell` lists
descendants, sends SIGTERM and waits up to 3 s for the exit again. If the old
agent ran that source, its recovery wait ended within 0.058 s. The receipt
does not record that phase, and the source of the old agent is not
established (see Identity). The observation is only that the receipt arrived
0.058 s after the deadline.

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
the trial when calibration does not finish.

Before it exits, the probe releases every blocker that it admitted and did not free. It records that count as
`admitted`. This also covers a calibration that reaches the cap of 512
blockers with every blocker held.

`evidence/macos-completion-k3/old_path.py` writes the plan, bounds and subject
hashes before the build. It first runs a cap check: a `free-1` probe with a
cap of 8 blockers must fail calibration and release all 8. It then runs five
rounds of the three conditions with a 3 s guest timeout and a 20 s bound for
each trial.

A trial is `completed-then-timed-out` when stdout is `ordinary-ok`, `timed_out` is true
and the exit is -1. A trial is `prompt` when the exit is 0 within 1 s. The
verdict is `defect` when every `free-1` trial completed and then timed out and
every control was prompt. The script exits 0 only when the verdict equals
`EXPECT`, the cap check passes, and every trial leaves no owned process or
capture file behind.

Each probe runs in its own session. When a probe reaches the trial bound, the
script records the start time and command of the probe and of each
descendant. It also records the capture files that those processes hold open.
It kills the process group and each identified process, and it moves the held
capture files into `OUT/NAME.captures/`. The cleanup bound is 5 s.

Every trial also compares the
`testanyware-exec-*` files in the per-user temporary directory before and
after the trial. A new file that no owned process held is unattributed. It
stays in place, and the trial is unclean.

The detector control `--variant single-hop` builds a copy of
`ProcessRunner.swift` in which the first worker polls `process.isRunning`
itself. The copy has no second block. It also replaces `waitUntilExit` with
`isRunning` polling, so it changes two things at once. It is a control for the
detector, not a proposed repair, and it does not locate the stalled phase.

### Results

All runs used swiftc 6.4.2 on the k19 host, with 16 active CPUs and
`kern.wq_max_constrained_threads` 80. Every calibration saturated at 80
workers, which equals that kernel limit.

| Run | Variant | `unsaturated` | `free-2` | `free-1` | Verdict |
| --- | --- | --- | --- | --- | --- |
| `old-path-01` | old | 5 prompt | 5 prompt | 5 completed-then-timed-out | `defect` |
| `single-hop-01` | single-hop | 5 prompt | 5 prompt | 5 prompt | `clean` |
| `old-path-03` | old | 5 prompt | 5 prompt | 5 completed-then-timed-out | `defect` |
| `single-hop-03` | single-hop | 5 prompt | 5 prompt | 5 prompt | `clean` |

The `-01` runs used the first probe and script. Their plans record the probe
sha256 `1c397196…47ea` and the script sha256 `5340695d…4514`. Integration k187
changed both files (see Review integration). The `-03` runs used the current
probe sha256 `82343d60…b2ac` and script sha256 `0a3dc4a4…17c0`. Their cap
checks passed, and their trials left no owned process or capture file.

Each `free-1` receipt of the old path has `stdout: "ordinary-ok"`,
`exit_code: -1`, `timed_out: true` and `stderr: "Process timed out after 3s"`.
That is the receipt of operation 8 with a 3 s timeout. The old path took
6.03–6.08 s inside the probe in `old-path-01` and 6.04–6.08 s in
`old-path-03`. That equals the 3 s wait plus the 3 s recovery wait after
SIGTERM. It is consistent with both waits reaching their timeouts. The probe
does not observe the child exit or either dispatch block, so it does not show
which phase stalled.

Subject hashes before and after each run are equal. The old build has source
sha256 `f9401669…6340` in every run. Its binary sha256 is `dcbf52fb…ba45` in
`old-path-01` and `178c8e99…777b` in `old-path-03`.

## Causes

| Candidate | Evidence for | Evidence against | State |
| --- | --- | --- | --- |
| Global-queue starvation delays the second block | Same receipt shape, reproduced 10 of 10 with the real code in two runs. The single-hop control is prompt, but it also replaces `waitUntilExit`. | The retained receipt arrived 0.058 s after the deadline, but the probe takes about 6 s. No one observed a guest starvation source or the stalled phase. In k186 the same agent build completed 22 of 22 requests. The guest limit is 64 constrained workers, and four concurrent requests need at most about 8. | Possible, not established |
| `waitUntilExit` missed the exit in the second block | None | If the old agent ran the current source, its recovery wait ended within 0.058 s. | Open, not tested |
| Capture-pipe EOF | None | `runShell` captures to files, not pipes, and stdout was complete. | Excluded for this path |
| Client deadline or app relay | None | The receipt came from the agent with `timed_out: true`, and the host exit was 0 within the 50 s bound. | Excluded as the source of the timeout |
| Guest load or scheduler stall | None | k186 saw no stall in 22 requests on an idle 4-CPU clone. No one observed the guest load at op 08. | Unknown |

The retained run gives no observation of the guest thread pool, the second
block or the child exit time. Those phases stay unknown.

## Design implications

A bounded wait must not depend on a second block in the same pool as the
waiter. Any repair belongs to a later leaf, and this slice adds no production
wait or cancel semantics. A repair must turn `diagnostic:macos-completion-old-path`
with `EXPECT=clean` green on the changed code. That result shows only that the
repair does not produce this receipt under this occupation. It does not prove
the cause of the retained failure.

## Regression invocation

```sh
mise exec -- task diagnostic:macos-completion-retained
mise exec -- task diagnostic:macos-completion-old-path OUT=<new directory>
mise exec -- task diagnostic:macos-completion-old-path OUT=<new directory> VARIANT=single-hop EXPECT=clean
mise exec -- task diagnostic:macos-completion-old-path OUT=<new directory> TRIAL_BOUND=4
mise exec -- task diagnostic:macos-completion-tools
```

The second command reports the known defect. It exits 0 when the verdict is
`defect`, so it stays outside the passing gate. Today `EXPECT=clean` on the old
variant exits 1.

The fourth command exercises the bounded cleanup. With a 4 s bound, each old
`free-1` probe is killed during its recovery wait. The verdict is `mixed` and
the command exits 1. Read its `summary.json` for `owned_resources_clean`.

## Owned-resource audit

The slice started no VM, app, clone or guest command. Each probe process
started one `bash` child. No trial of a `defect` or `clean` run reached the
20 s bound. In `old-path-03` and `single-hop-03`, every trial left no process
in its group and no new capture file. The per-user temporary directory held 0
`testanyware-exec-*` files before and after the `-03` runs and
`cleanup-check-02`.

`cleanup-check-02` ran the old variant with a 4 s bound. All five `free-1`
probes reached the bound. For each one, the script identified the probe,
found its two open capture files, killed it, and moved the files into the
run. The `.out` files hold `ordinary-ok`. No process survived, no file was
unattributed, and each cleanup took at most 0.28 s.

The slice ran no process cleanup by name.

## Review integration

Integration k187 assessed the three findings of review k184.

- F1, completion and phase claims. Accepted. The report no longer states
  that the retained recovery wait succeeded or that the local recovery wait
  starved. The single-hop control is described as a change of two things.
  Guest phase observation stays with the live work below.
- F2, release after a failed calibration. Accepted. The probe released
  0 blockers when calibration reached the cap with every blocker held. It
  now releases every admitted blocker that it did not free, and it records
  `admitted`. The cap check exercises that path in every run.
- F3, ownership after the trial bound. Accepted. The script killed only the
  direct probe, so capture files and any descendant could remain. The script
  now uses the containment that Mechanism under test describes.

An intermediate repair redirected `TMPDIR` to a directory in each run. Darwin
Foundation ignores `TMPDIR` for `FileManager.default.temporaryDirectory`.
`cleanup-check-01` used that repair. It reported clean, but its five bounded
trials left 10 capture files in the per-user temporary directory. Integration
k187 moved those files into `cleanup-check-01/leaked-captures/`.

`old-path-02` and
`single-hop-02` used the same intermediate script. Their verdicts are `defect`
and `clean`, and their capture audits are not valid.

## Owned process fixtures

Leaf k185 replaced the `pkill -f 'sleep 30'` cleanup in
`ProcessRunnerTests.swift`. The two tests that used it now start their
`sleep 30` through an owned fixture,
`agents/macos/Tests/TestAnywareAgentTests/OwnedProcessFixture.swift`. The test
shell writes the PID of the `sleep` into a file in a directory that only one
fixture uses.

The fixture records the PID and the kernel start time of that process. It accepts the process only when it started after the fixture and
runs `sleep`. A teardown block sends SIGTERM, waits 2 s, then sends SIGKILL
and waits 2 s. It signals only while the PID still carries the recorded start
time. The leaf did not change `ProcessRunner.swift` (sha256 `f9401669…6340`).

`task diagnostic:macos-process-fixtures` starts an unrelated `sleep 300` beside
the suite. The broad pattern selects that control in every run. `ARM=owned`
runs `agent:macos:test` on the working tree. `ARM=broad` is the fault control.
It runs `swift test` on a temporary copy of the package with the test file of
`990641750f` (sha256 `07fac70a…137c`). The fault arm refuses to run while
another process of this user matches `sleep 30`.

| Run | Arm | Runs | Control survived | Fixture outcomes | Suite runs with a failure |
| --- | --- | --- | --- | --- | --- |
| `owned-01` | owned | 5 | 5 | 10 `stoppedByTerm` | 3 |
| `owned-02` | owned | 15 | 15 | 30 `stoppedByTerm` | 4 |
| `broad-01` | broad | 5 | 0 | none | 1 |
| `broad-02` | broad | 15 | 0 | none | 3 |

No owned run left a new process that the broad pattern matches. The subject
hashes were the same before and after every run. The audit in each `plan.json`
finds no `pkill` or `killall` in the agent package source. The same scan finds
two in the baseline test file.

### Review k188 and repairs in k189

Review k188 found five gaps in the k185 fixture and control. Leaf k189
verified each gap in the source and repaired it. The retained runs above are
unchanged.

- Ownership. The fixture could adopt any process that held the PID after
  fixture creation. That included the xctest process itself, which can start
  less than 1 s before the fixture. A PID file that named xctest made cleanup
  send SIGTERM to the test runner. The fixture now has three conditions. A
  child must start after the fixture and run in the test session. It must be
  neither the test process nor an ancestor. Its identity must also stay the
  same across the check. A process that fails the binding is refused and never signalled.
- Cancellation. A child that `track` had not bound was outside teardown.
  `pidFile` now registers each name, and `cleanUp` resolves each name that
  `track` did not bind. A name that cleanup cannot resolve fails the test.
- Fault arm. One scan before the build could not protect a shared host, and a
  failed `pgrep` counted as no match. A failed scan now stops the script. The
  script also refuses a visible match before each run. The broad arm runs
  only with `ISOLATED_HOST=1`, because a process can still start between a
  scan and the restored `pkill -f`. Only an isolated host closes that window.
- Exercise. A run now counts only when the suite finishes inside its bound
  and both fixture cases finish. In the owned arm, the run also needs one
  cleanup receipt from each case and no survivor. Otherwise its verdict is
  `invalid-exercise`. A plan with no runs fails. All 40 retained runs pass
  this check.
- Control cleanup. The script now stops and reaps the control on every exit
  after the control starts. It records the error and any cleanup failure.

`OwnedProcessFixtureTests` holds four cases: the test process, an older
process, a process in another session and a cancelled `track`. In fault
injection, a suite that failed to start and a failed `pgrep` after the control start
each left the control stopped and reaped. Each also left the original error in
`run.json`. Two live owned runs in `owned-k189` gave `survives` with no
exercise problem and no suite failure.

Two limits remain. Darwin has no process handle, so the kernel can reuse a
PID between the identity check and `kill`. A process of the same session can
also start after the fixture and take the PID of an exited child before
`track` reads it. Both need a PID reuse inside one test.

### Intermittent suite failure

The suite failed in 7 of 20 owned runs and in 4 of 20 broad runs. Fisher's
exact test gives p = 0.48, so these runs do not show a difference between the
arms. Every failure is in `testHandlesLargeOutputWithoutPipeDeadlock` or
`testCapturesStdoutAndExitCode`. Each failing test reports exit -1 and
`timed_out` after its full timeout of 10 s or 5 s. The same commands complete
in about 0.07 s.

`testCapturesStdoutAndExitCode` runs before the two fixture tests. It failed in
two owned runs and in one broad run, so its failures come before any fixture
code runs. The fixture tests passed in all 20 owned runs.

These receipts have the completed-then-timed-out shape of the retained
failure, from the unchanged `runShell` on the host in the normal suite. This
leaf did not inspect the dispatch pool, so the cause of these timeouts is not
established. The suite is not green until k3 resolves the defect.

## Live clone run (k186)

Leaf k186 ran the exact issue #14 request on one fresh owned clone of
`testanyware-golden-macos-27`. Run `k186-completion-macos-01` froze its plan
in `inventory.json` before the first live call and ran once. The source was
commit `4b23ff10`. The dirty diff holds only the run's own outer records.

The plan ran these calls in this order:

1. Agent health.
2. Ten serial `file exec --timeout 30 --json "printf ordinary-ok"` requests.
   Each has a host bound of 50 s.
3. Guest identity: agent hash, macOS version and architecture.
4. Three rounds of four concurrent exact requests from `concurrent_k186.py`.
   Each request has its own host bound of 50 s, then TERM 2 s and KILL 2 s.
5. A guest pool read: CPU counts, workqueue limits and agent threads.
6. Agent health.

The checker `completion_k186_check.py` judges each receipt. A sample passes
when the host exits 0 within 50 s and the receipt has `exit_code` 0,
`timed_out` false, stdout `ordinary-ok` and empty stderr. Review k190 found
gaps in this checker and in the outer tools. Leaf k191 repaired them. See
Review k190 and repairs in k191.

### Results

| Workload | Samples | Passed | Host seconds, minimum to maximum |
| --- | --- | --- | --- |
| Serial | 10 | 10 | 0.106 to 0.310 |
| Concurrent, round 1 | 4 | 4 | 0.142 to 0.146 |
| Concurrent, round 2 | 4 | 4 | 0.258 to 0.270 |
| Concurrent, round 3 | 4 | 4 | 0.145 to 0.149 |

The four starts of each round lie within 0.004 s of each other. No request
reached its host bound, and the wrapper sent no signal.

The table gives the verdicts of the k186 checker. The k191 checker judges the
same records again. The ten serial samples pass. The twelve concurrent samples
are unknown, because the k186 wrapper did not record EOF on their pipes.

A clean run of this size bounds the failure rate only loosely. The figures in
this paragraph are conditional illustrations, not empirical bounds. Each
figure assumes independent requests with one common failure probability. This
run does not establish that model. Serial and concurrent requests ran on one
guest, and the k3 candidate is a shared dispatch pool, which can correlate
failures.

Under the model, a failure probability of 0.1 per request, as in the
retained run, gives 22 clean requests with probability 0.098 (`0.9**22`). The
one-sided 95 % bound after 0 of 22 is 0.127 (`1 - 0.05**(1/22)`). The bound
after the 10 serial requests alone is 0.259 (`1 - 0.05**(1/10)`). The 12
concurrent requests are a different exposure, so the serial figure does not
cover them. If every request failed together with probability 0.1, all 22
would succeed with probability 0.9.

### Phases

| Phase | Observation in k186 |
| --- | --- |
| Child exit | Exit 0 in every receipt |
| Wait completion | Before the guest deadline in every sample (`timed_out` false). The instant is unknown. |
| Capture read | Complete: stdout `ordinary-ok` and empty stderr in every receipt |
| Handler response | Every request returned one agent receipt |
| Client deadline | Not reached. The longest request took 0.310 s against 50 s. |
| App relay | Unknown. No existing seam observes it separately. |

### Guest identity and pool

| Subject | Identity |
| --- | --- |
| Guest agent | `/usr/local/bin/testanyware-agent`, sha256 `1282f4cf…8a84`, the same build as the retained failure |
| Guest OS | macOS 27.0.1, arm64 |
| Guest pool | `hw.ncpu` 4, `hw.activecpu` 4, `kern.wq_max_threads` 512, `kern.wq_max_constrained_threads` 64 |
| Host CLI | 2.8.0, sha256 `44b08089…a340` |
| App executable | sha256 `2be50799…e91c`, CDHash `1997a620…6b96`, team `TA43A4RUP3` |
| Bundled agent | sha256 `22dd56ce…b9f5`, not the guest agent |
| Host pool | `hw.ncpu` 16, `kern.wq_max_constrained_threads` 80 |

The host CLI and the app executable differ from the retained run. The guest
agent is the same build.

The pool read did not observe the agent threads. In the guest, `pgrep -x
testanyware-agent` printed no PID, so `ps -M -p` failed with a usage error.
On the host, `pgrep -x` matches a 17-character binary name, so name truncation
does not explain the miss. The cause is unknown. The plan ran once, and k186
did not repeat the read.

### Instrumented clone

Leaf k186 ran no instrumented clone. The task file declared the condition
before the run: a failed sample, or a constrained limit near the demand of the
workload. Neither held. A probe of the dispatch pool would have had no stalled
request to observe. The k3 candidate needs every constrained worker but one
to be busy, which is 63 of 64 in this guest. The k186 workload asked for at most about
8.

### Owned-resource audit

| Check | Result |
| --- | --- |
| Clone | `k186-macos-b1256cae4068418dac994e03f0392f6b`, acquired in 15.238 s |
| Guest cleanup | Note only. Each request exited, and disposal removes the guest. |
| Disposal | `disposed`, 3.920 s. The inventory checker then proved absence in 0.147 s. |
| Ledger | One resource, state `disposed`, no residual |
| Public VM inventory | Identical before and after |
| Tart inventory | Same entries. Only the `Accessed` time of the source golden changed. |
| App process | PID 1488 before and after |
| Subjects | Every digest equal before and after |
| Writer | 89 frames sent, 89 acknowledged, 0 failed, 0 dropped |

The run touched no unrelated guest and booted no golden.

### Regression invocation

`mise exec -- task diagnostic:macos-completion-live OUT=<new path>` boots one
owned clone and runs the frozen plan. Since k191, the task exits 1 when the
probe fails, the verdict is not true, the receipt check fails or a tool digest
changes. `diagnostic:macos-completion-live-check RUN=<run dir>` judges a
retained run and exits 0 only for verdict `pass`.
`diagnostic:macos-completion-live-tools` checks the tooling and runs its
controls without a VM. The live task needs the TestAnyware app and the
retained golden.

### Review k190 and repairs in k191

Review k190 inspected the k186 tools and records. It reported six findings at
P2. Leaf k191 reproduced or read each one and accepted all six. No
finding changes the 22 retained receipts. Each one is a JSON object with the
expected fields. Leaf k191 ran no live diagnostic and replaced no k186 record.

| Finding | Disposition and repair |
| --- | --- |
| F1: a JSON `null` receipt passes | Reproduced: `judge(0, 0.1, b"null")` returned `pass`. The checker now needs one object with each expected field. |
| F2: absent samples and controller failure pass | Read in the source. The checker now needs the frozen shape and unique names and indices. It also checks the host evidence. |
| F3: capture ends at the leader reap | Read in the source. The wrapper now reads each pipe until EOF, for at most 1 s after the last reap. It records `stdout_eof` and `stderr_eof`. |
| F4: the live task exits 0 after a failure | Read in the source: the runner ended with `cat`. The probe and the runner now exit 1 on each failure. |
| F5: the checker and runner have no digests | Read in the source. Both are now controller subjects. The runner also hashes ten tools before the probe and after the check. |
| F6: the bounds assume independent requests | Accepted. Results now states the model and marks the figures as conditional. |

The checker now has three verdicts: `pass`, `fail` and `unknown`. A sample
fails when a condition is false. A sample is unknown when no condition is
false but EOF on a stream has no record. The run fails when a sample fails, a
sample is missing or duplicate, or the host evidence is not healthy. Healthy
host evidence has these properties:

- `result.json` has `ok` true and no unreached call.
- No inspection failure, and equal non-empty subject digests.
- No failed, dropped or abandoned storage frame, and an orderly storage close.
- Each serial and wrapper call exits 0 with EOF on both streams.

The k191 checker judged the retained run as `unknown`, with exit 1. The ten
serial samples pass. The twelve concurrent samples are unknown, and the host
evidence has no failure. The record is
`experiments/exec-reliability/evidence/k191-recheck/check.json`.

The concurrent budget grows from 56 s to 57 s, to cover the 1 s capture grace.
The source changes, so the next live run has a new frozen-source commit.

The controls in `test_completion_k186.py` need no VM. They break each property
above and expect a refusal. A fake CLI leaves a descendant that writes the
second half of the receipt after the leader exits. Fake outer commands drive
the runner through each failure exit.

Against the k186 wrapper, runner and probe, all seven of their controls fail.
With the k191 tools, all 32 controls pass. The k186 checker has a different
`judge` signature, so the checker controls do not run against it. Inspection
and the F1 reproduction supply its failures.

Two limits remain. The k186 run has no digests of its checker or runner
through the check, and no record supplies them. All 21 recorded subject
digests match. The wrapper keeps a capture grace of 1 s, so a descendant that holds
a pipe longer makes the sample fail, not wait.

## Remaining k3 work

These items of k3 remain for later leaves:

- The cause of the retained op 08 timeout. A clean run of 22 requests does not
  establish or exclude any candidate.
- Guest observation of the agent threads, the child exit and the second block
  during a failing request. The k186 pool read missed the agent process.
- The intermittent timeouts of `agent:macos:test` on the host (see Owned
  process fixtures). That 16-CPU host has a limit of 80 constrained workers.

## Lifecycle owner result (k219)

Leaf `macos-agent-lifecycle-k219` replaced the old runner with a lifecycle
owner. The owner runs each command on its own thread and waits on a kqueue
source, so no dispatch pool worker waits for the command. The section
"Design implications" above names the repair condition. The evidence is in
`experiments/exec-reliability/evidence/macos-agent-lifecycle-k219`.

| Run | Runner source sha256 | `EXPECT` | Verdict | `free-1` trials |
| --- | --- | --- | --- | --- |
| `completion-base-defect-01` | `6a3c3846…`, the base commit | `defect` | `defect` | 5 of 5 completed, then timed out after 6.8 s |
| `completion-new-clean-02` | `a562fc62…`, the owner | `clean` | `clean` | 5 of 5 prompt, near 0.73 s |

The base runner differs from the source of the retained runs `old-path-01`
to `old-path-03`, `f9401669…`. A temporary jj workspace at the base commit
therefore built it again. The earlier owner `d4d7d783…` also gave `clean`, in
`completion-new-clean-01`.

As the section "Design implications" states, this result shows that the owner
does not produce this receipt under this occupation. It does not prove the
cause of the retained op 08 failure, which stays open.

`agent:macos:test` passed 20 of 20 runs on the owner. The intermittent
timeouts of the old `runShell` therefore did not recur in the host suite.
An earlier loop on `d4d7d783…` failed 3 of 20 runs for a different reason.
Its group probe stopped at a transient EPERM after SIGKILL. The owner now
probes until ESRCH or its bound.

The fixture self-test gave one more observation. At the base commit,
`OwnedProcessFixtureTests` hung in 5 of 12 runs. `Process.waitUntilExit()`
blocked after Foundation had reaped the Perl control process, and no process
remained. This shows that the Foundation wait can block after an exit in a
test process. It does not show that the old agent missed an exit, so the
candidate "`waitUntilExit` missed the exit in the second block" stays open.

### Review k233 and repairs in k234

Review `macos-agent-lifecycle-k233` inspected the k219 owner and reported four
findings from source. Integration `macos-agent-lifecycle-k234` checked each
finding against the source and the spec. The evidence is in
`experiments/exec-reliability/evidence/macos-agent-lifecycle-k234`.

| Finding | Disposition | Control |
| --- | --- | --- |
| R1, an exit event before a waitable status leads to a blocking reap | Repaired | `testExitEventWithoutStatusIsNotAnExit` |
| R2, a large accepted timeout traps after the spawn | Repaired | `testHugeTimeoutDoesNotOverflowTheDeadline` |
| R3, finalisation work after its deadline grows with the output | Control repaired, rest accepted as a limit | `testStalledCaptureEndsAtTheFinaliseDeadline` |
| R4, a failed exit registration waits out the whole budget | Repaired | `testFailedRegistrationStillSeesThePromptExit` |

The waiter now decides an exit only from a `waitid` status check with
`WNOWAIT`. Three cases make the waiter poll the status until the deadline:
an exit event before the status, a failed registration and a queue error. A narrow registration seam in `KqueueLeaderWaiter` drives the R1 and
R4 controls. The R1 control adds a timer that fires at once in place of the
process source. The R4 control reports `ENOMEM` and adds no source.

The deadline arithmetic now saturates at `UInt64.max`, and each `kevent` wait
lasts at most 60 s. The request keeps its accepted range, so an old Host CLI
sees no new error. The unchanged owner `a562fc62…`, built alone with a probe,
exited with status 133 (SIGTRAP) at a timeout of 20000000000 s
(`r2-old-path`).

Each R1 and R4 control failed under a mutation that restored the old
behaviour (`mutation-controls`). With the exit event taken as an exit, the
owner blocked in the reap and reported `exited` for a command that had to
time out. With the registration error ignored, a 0.1 s command took 30000 ms.

R3 is partly a limit of the contract. Publishing an output of N bytes takes
time in proportion to N, so no read deadline bounds an unbounded output. The
owner already counts the base64 and legacy work in `finalisedAfterMs`. The
spec section "Limits" now states this limit. The stalled-read control accepted
a time of 2300 ms against the 2000 ms bound of the spec. It now asserts that
bound.

The script `measure.sh` measured the repaired owner `304b9bc4…` in sequence,
with equal subject digests before and after. The k3 detector gave verdict
`clean` with five prompt `free-1` trials near 0.74 s. The k12 run gave exit 0
for the run, the check and the record judge. `agent:macos:test` passed 20 of
20 runs of 35 tests. These are lower-level controls. The ten-clone guest
acceptance stays **0/10, unavailable**.
