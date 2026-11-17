# Linux timeout lifecycle

Leaf `linux-timeout-lifecycle-k5` freezes a finite matrix of exec outcomes on
the Linux agent. It records what each layer reports for each outcome. The layers
are the agent handler, the agent HTTP response and the Host CLI receipt. The
leaf also observes the processes of each case after the timeout.

The leaf runs the unchanged agent source in fresh containers of one pinned image.
It uses no clone and no VM. The live clone run moved to leaf
`linux-timeout-live-k204`. The leaf makes no repair. Leaf `lifecycle-evidence-k8`
receives this evidence.

## Summary

- The reference seam reports every case faithfully. Its deadline phases stay
  within the 2 s grace and the 2 s kill bound.
- The unchanged handler kills only the shell, with SIGKILL and no TERM. A child
  of the shell survives the timeout. The survivor stays in the group of the agent.
  This reproduces the fork/wait survivor of retained failure B.
- A deadline, a decode failure, a hangup and a spawn failure all come back as exit
  status -1. The reply has no timeout flag.
- The handler drops the partial output of a deadline. `TimeoutExpired` held the
  bytes, and the handler replaced them with `Process timed out` in stderr.
- The handler trims each trailing line feed and refuses bytes that are not UTF-8.
  It reports a signal death as the negative signal number.
- The CLI text mode exits 255 for each synthetic -1, so it cannot tell a deadline
  from `exit 255`. The CLI JSON mode reports `timed_out: false` on every deadline.

## Matrix

Each case is a `bash -c` command. The timeout is 2 s for the four deadline cases
and 10 s for the others. The oracle bytes are the bytes of the root brief.

| Case | Command | Faithful outcome |
| --- | --- | --- |
| `oracle-text` | `printf 'Explain this change.'`, then stderr `separate stderr\n` | exit 0, stdout `4578706c61696e2074686973206368616e67652e`, stderr `7365706172617465207374646572720a` |
| `binary` | `printf '\000\377\200A\n'` | exit 0, stdout `00ff80410a` |
| `exit-23-partial` | one line to each stream, then `exit 23` | exit 23, both lines with their line feeds |
| `exit-255` | one line, then `exit 255` | exit 255 |
| `signal-kill` | one line, then `kill -KILL $$` | signal 9 |
| `signal-hup` | one line, then `kill -HUP $$` | signal 1 |
| `deadline` | one line to each stream, then a unique `sleep 30.…` | deadline, TERM ends the group, partial bytes kept |
| `deadline-term-ignored` | `trap '' TERM`, one line, then a unique sleep | deadline, KILL after the 2 s grace, partial bytes kept |
| `issue-sleep` | `sleep 120` | deadline, TERM ends the group |
| `issue-fork-wait` | `sleep 120 & p=$!; echo $p > /tmp/vmrel-k16/child.pid; wait` | deadline, TERM ends the group and the child |
| `spawn-failure` | `printf 'never runs\n'` under `RLIMIT_NPROC` of 1 | spawn error, errno 11 |

<!-- tc: depart long-sentence because a table cell names a command and its oracle, and splitting it loses the pairing -->

The two issue commands are the exact controls of retained failure B. Each seam
creates `/tmp/vmfit-k15` and `/tmp/vmrel-k16` first, as that run did. The
spawn failure carries the label `instrumentation: rlimit-nproc`.

## Seams

- `reference` runs each command with `subprocess.Popen` in a new session. It
  waits with `os.wait4`. At the deadline it sends TERM to the group, waits 2 s,
  and then sends KILL. It is the oracle for bytes, status and phases.
- `process` calls the unchanged `system_endpoints.handle_exec` once per case.
- `traced` does the same. It wraps `Popen.communicate`, `Popen.send_signal`,
  `Popen.wait` and `subprocess.run`. Each wrapper calls the original and records
  a time. The `run` wrapper also records the partial bytes of `TimeoutExpired`.
- `http` posts each case to a fresh agent container over loopback. It keeps the
  raw status, headers and body bytes.
- `cli-text` and `cli-json` run the checkout CLI against a test-mode app. A Linux
  VM spec points each call at its own fresh agent container.

The three inside seams run in one container each, as uid 1000, with
`--network none`. The host seams use one agent container per case and seam, 33
in all. Every container runs the unchanged `agents/linux` source from a
read-only mount. Two small stubs stand in for the desktop modules `gi` and
`pyatspi`. The exec path does not use them.

## Ownership and observation

Each container is fresh and carries the label `testanyware.k5.run=<run tag>`. A
process belongs to a case when the case container did not list it before the
case. The seam compares the PID and the start time in clock ticks, so a reused
PID is not owned.

The seam lists the processes of the container at the return of the case. It
lists them again at the faithful bound. For a deadline case, that bound is the
start plus the timeout, the grace and the kill bound, which is 6 s. For other
cases it is 2 s after the return. After the bound, the seam sends SIGKILL to each
owned live process whose start time still matches. Then it lists the processes
once more.

In `run-03`, the cleanup read the start times once and then called `kill` by
PID. If the kernel gave the PID to a new process between that read and the
signal, the new process received the SIGKILL. The run shows no such reuse, but
the rule did not exclude it.

Leaf k206 replaced the cleanup. It opens a pidfd first and then reads the start
time again. The kernel refuses the signal when the pidfd process is dead, so a
later holder of the PID cannot receive it. The identity still has one limit. If a
new process gets the PID within the clock tick of the target start, both
processes have the same identity.

## Results

Attempt `run-03` is the complete sample. Its run tag is `477956470135`. Every
field below comes from `run-03/check.json` and the call stdout beside it. The
process, HTTP and CLI JSON seams agree on each status and each stream, so one
line names all three.

#### `oracle-text`

- Reference: exit 0, both oracle streams exact.
- Old path: exit 0. Stderr lost its trailing line feed:
  `736570617261746520737464657272`.

#### `binary`

- Reference: exit 0, stdout `00ff80410a`.
- Old path: exit -1, empty stdout. Stderr is
  `'utf-8' codec can't decode byte 0xff in position 1…`. The handler asks for
  text, so the decode fails inside `communicate`. `subprocess.run` then kills the
  shell. The traced seam records that SIGKILL.
- CLI text: exit 255.

#### `exit-23-partial`

- Reference: exit 23, `partial out\n` and `partial err\n`.
- Old path: exit 23, both streams without their line feeds.

#### `exit-255`

- Reference and old path: exit 255. The old path trims the line feed.
- CLI text: exit 255, the same as each synthetic -1.

#### `signal-kill`

- Reference: signal 9.
- Old path: exit status -9, the `returncode` of Python. CLI text exits 247.

#### `signal-hup`

- Reference: signal 1.
- Old path: exit status -1, the same as a deadline. CLI text exits 255.

#### `deadline`

- Reference: TERM at 2.001 s, reap 0.003 s later, end at 2.004 s.
  Stdout `before deadline\n`, stderr `deadline err\n`.
- Old path: exit -1 at about 2.01 s, empty stdout, stderr `Process timed out`,
  no timeout flag. The traced seam shows SIGKILL first and the reap 0.5 ms later.
  `TimeoutExpired` held stdout `6265666f726520646561646c696e650a` and stderr
  `646561646c696e65206572720a`, and the handler dropped them.

#### `deadline-term-ignored`

- Reference: TERM at 2 s, KILL after a 2.002 s grace, end at 4.004 s. Stdout
  `ignoring term\n`.
- Old path: the same reply as `deadline` at about 2.01 s. The handler sends no
  TERM, so the trap has no effect. `TimeoutExpired` held stdout
  `69676e6f72696e67207465726d0a`.

#### `issue-sleep`

- Reference: TERM at 2.001 s, end at 2.004 s, no survivor.
- Old path: exit -1, `Process timed out`, no timeout flag. CLI JSON reports
  `timed_out: false`. CLI text exits 255. The shell `exec`s `sleep`, so the KILL
  reaches it, and no process survives.

#### `issue-fork-wait`

- Reference: TERM to the group at 2.001 s, end at 2.004 s. The child dies
  with the shell. The
  child PID file held `18`.
- Old path: the same reply as `issue-sleep`. The background `sleep 120` survives
  in every seam, at the return and at the 6 s bound. Its parent is the
  container init, PID 1. Its group is the group of the process that called the
  handler.

| Seam | Survivor PID | PPID | PGID | Cleanup |
| --- | --- | --- | --- | --- |
| process | 18 | 1 | 7 | 0.11 s |
| traced | 18 | 1 | 7 | 0.11 s |
| http | 22 | 1 | 8 | 0.38 s |
| cli-text | 21 | 1 | 7 | 0.38 s |
| cli-json | 21 | 1 | 7 | 0.36 s |

In the host seams, the agent is PID 7 or PID 8 and leads its own group. The
survivor PGID equals the agent PID in each container. In the inside seams, PGID 7
is the group of the seam process, which listed itself out of each observation.

#### `spawn-failure`

- Reference: spawn error, errno 11.
- Old path: HTTP 200 with exit -1 and stderr
  `[Errno 11] Resource temporarily unavailable`. The errno survives only as text.
  CLI text exits 255.

### Phase times

| Seam | Case | First signal | Grace | Kill to reap | End |
| --- | --- | --- | --- | --- | --- |
| reference | `deadline` | TERM | 0.003 s | none | 2.004 s |
| reference | `deadline-term-ignored` | TERM | 2.002 s | 0.000 s | 4.004 s |
| reference | `issue-sleep` | TERM | 0.003 s | none | 2.004 s |
| reference | `issue-fork-wait` | TERM | 0.003 s | none | 2.004 s |
| traced | `deadline` | KILL | none | 0.0005 s | 2.005 s |
| traced | `deadline-term-ignored` | KILL | none | 0.0004 s | 2.005 s |
| traced | `issue-sleep` | KILL | none | 0.0005 s | 2.014 s |
| traced | `issue-fork-wait` | KILL | none | 0.0005 s | 2.005 s |

The old path is fast because it kills one process and waits for nothing else.
The reap ends the handler while a child of the shell still runs.

## Defects of the old path

The checker names each loss. `--expect-defects` passes only when the sound
evidence shows every frozen defect, so a repaired agent makes it fail.

| Defect | Cases |
| --- | --- |
| `trailing-lf-trimmed` | every case with a final line feed |
| `non-utf8-dropped` | `binary` |
| `partial-output-dropped` | `deadline`, `deadline-term-ignored` |
| `timeout-text-in-stderr` | each deadline case |
| `signal-reported-as-exit` | `signal-kill`, `signal-hup` |
| `synthetic-exit-code` | `binary` and each deadline case |
| `deadline-not-flagged` | each deadline case |
| `spawn-failure-unmapped` | `spawn-failure` |
| `ambiguous-status` | -1 for deadline, decode, hangup and spawn, and 255 on CLI text |
| `term-skipped` | traced `deadline`, `issue-sleep`, `issue-fork-wait` |
| `survivor-at-return`, `survivor-at-bound` | `issue-fork-wait` in every old-path seam |

## Regression candidates

Each item below is a frozen check that a repair must turn from failing to
passing. The ordinary suite does not run them, so no test waits for the repair.

- `issue-fork-wait` at the process seam: no owned process at the 6 s bound.
- `deadline` at the process seam: a timeout flag, the partial bytes, and TERM to
  the group before KILL.
- `deadline-term-ignored`: KILL after a grace of 2 s at most.
- `binary` and `exit-23-partial`: the exact bytes.
- `signal-kill` and `signal-hup`: a signal outcome, not an exit status.
- `spawn-failure`: a spawn error with errno 11.
- `exit-255` on CLI text: an exit status that a deadline cannot share.

`task diagnostic:linux-timeout-lifecycle-old-path OUT=…` runs the detector on
one run directory.

## Causal limits

- The containers share one kernel with Docker Desktop. A VM guest has its own
  kernel, systemd and session. The handler code is the same, but the init of a
  container is `docker-init`, not systemd. Leaf k204 tests the clone.
- The spawn failure uses `RLIMIT_NPROC`. A real guest can fail to spawn in other
  ways, such as a missing shell. The errno can then differ.
- The handler of Python 3.12.15 is the subject. A guest with another Python
  version can time out in another way.
- The host seams each start a fresh agent. A long-lived agent can hold earlier
  survivors in its group, which this leaf does not test.
- The leaf did not measure the timing cost of the traced seam. The seam does
  not change arguments or results.
- The leaf ran one sample per case. It never replaced a failed sample.

## Owned-resource audit

| Resource | Owner proof | Bound | Outcome |
| --- | --- | --- | --- |
| 3 inside containers | name `k5-<tag>-inside-<seam>`, run label, `--rm` | 180 s per call | each exited 0, gone |
| 33 agent containers | name `k5-<tag>-<seam>-<index>`, run label | disposal 45 s | removed in 0.84 s |
| case processes | absent from the case baseline, same start ticks | cleanup 30 s | 5 survivors killed, at most 0.38 s each |
| test-mode app | child of the host seam, start time | stop 2 s | stopped by TERM, exit 0 |
| host root under `/tmp` | created by the host seam | end of call | removed |
| inside work directories | created by each inside seam | end of call | removed |

After the plan, the driver listed and removed the containers with the run label.
It found none. Each inside container had only `docker-init` at its final
listing. The
controller saw each call group end with only its zombie leader.

## Attempts

- `run-01`: the controller refused the plan before any call ran. The inside
  calls named a bare `docker`. The refreeze resolved an absolute path.
- `run-02`: the three inside seams ran, and their defects equal those of
  `run-03`. The host call failed at import, because `-I` keeps the script
  directory off `sys.path`. The refreeze starts the host seam with `-E -s -B`.
- `run-03`: complete and sound. Both checker modes exit 0.

Each refreeze is its own commit, made before the next attempt.

## Provenance

- Freeze commit of `run-03`: `5e828cb6da55afe266064b81a022aa63472da697`.
- Image: `python@sha256:dddfd7e07f9d15aeeca61529320492139d21cac7f0070c00609243e51e4e0016`,
  linux/arm64, Python 3.12.15, bash 5.2.37. Docker 29.9.3.
- The agent source in the image mount is the checkout `agents/linux`. Its
  `server.py` and `system_endpoints.py` are byte-identical to the retained
  `diagnosis-linux-01` copies.
- `run-03/inventory.json` records the hashes of the sources, the executables and
  the lockfiles, and the frozen plan.
- In `run-03`, the controller hashed only part of the sources before and after
  the run. The agent modules other than `server.py` and `system_endpoints.py`
  and the two stubs have a hash from before the run only. The checker import
  `outcomes_k12_check.py` has no hash. These records cannot be completed after
  the run. Leaf k206 made the plan hash the whole local import closure, the
  agent package and the stubs, before and after the run.

## Evidence

`experiments/exec-reliability/evidence/linux-timeout-lifecycle-k5/` holds the
three attempts. Each holds `inventory.json`, `summary.json` and the controller
run directory. `run-02` and `run-03` also hold `check.json`.

## Regression invocation

```sh
task diagnostic:linux-timeout-lifecycle OUT=/path/to/new/out
task diagnostic:linux-timeout-lifecycle-check OUT=/path/to/new/out
task diagnostic:linux-timeout-lifecycle-old-path OUT=/path/to/new/out
task diagnostic:linux-timeout-lifecycle-tools
```

The run needs Docker and the pinned image. It pulls nothing.

The checker reads the run directory inside `OUT`, by the base name of the path
in `summary.json`. The recorded absolute path stays as provenance only. So a
copied bundle replays from its own bytes. In leaf k206, a replay of a copy of
`run-03` wrote a `check.json` that is byte-identical to the retained one.

## Live clone run (leaf k204)

Leaf `linux-timeout-live-k204` ran the timeout controls on one unchanged clone of
`testanyware-golden-linux-24.04`. Each control had a guest timeout of 2 s. Each
control ran once through the Host CLI and once through the raw agent. The plan
then ran the retained r5 VM-fit sequence as its own case. The leaf used no
instrumented clone, because the receipts and the own observations settled each
question. The leaf makes no repair.

### Result

- Failure B reproduces on the clone, through both paths. After each fork/wait
  timeout, the shell was gone and its `sleep 120` child was still listed. The
  child was still listed in a second observation, 5 s later.
- Each surviving child moved to `/usr/lib/systemd/systemd --user` as its parent.
  It stayed in the process group and the session of the agent. The handler kills
  one PID, so the group of the agent keeps the survivor.
- The exact issue command `sleep 120` left no process. `bash -c` runs a single
  command in place of the shell, so the kill of the shell ends `sleep`. Only the
  fork/wait control can show failure B.
- The retained r5 case reproduces its own survivors. Its timeout child and its
  cancel child were both listed after their calls returned.
- Each timeout came back as exit status -1 with `Process timed out` in stderr.
  The CLI JSON receipt reported `timed_out: false`. The raw reply has no
  timeout flag.
- The receipts dropped the partial output. Each fork/wait control printed a line
  to stdout and to stderr before the timeout. Each receipt has an empty stdout.
  The partial file of each control downloaded intact, 13 bytes.
- No call after a timeout stalled. The checker found the stall not reproduced.

### Per-case evidence of attempt 02

The table gives the host time of each call. The overhead is the host time
minus the 2 s guest timeout.

| Call | Path | Receipt | Host time | Overhead |
| --- | --- | --- | --- | --- |
| `cli-sleep` | CLI | exit -1, `Process timed out`, `timed_out: false` | 2.074 s | 0.074 s |
| `raw-sleep` | raw | HTTP 200, `exitCode` -1, `Process timed out` | 2.111 s | 0.111 s |
| `cli-fork` | CLI | exit -1, `Process timed out`, `timed_out: false` | 2.097 s | 0.097 s |
| `raw-fork` | raw | HTTP 200, `exitCode` -1, `Process timed out` | 2.115 s | 0.115 s |
| `r5-timeout` | CLI | exit -1, `Process timed out`, `timed_out: false` | 2.097 s | 0.097 s |
| `r5-cancel` | CLI | host interrupt at 2 s, TERM to the CLI, exit -15 | 2.108 s | none |

The raw client measured 2.025 s for `raw-sleep` and 2.023 s for `raw-fork`.

The agent was `python3 -m testanyware_agent`, PID 1142, with parent 1130. PID
1130 is `systemd --user`. The agent led its own group and session, 1142.

| Observation | Time after the control | Shell | Child |
| --- | --- | --- | --- |
| `observe-cli-fork` | 0 s | 2018 not listed | 2020 listed: parent 1130, group 1142, session 1142 |
| `observe-raw-fork` | 0 s | 2026 not listed | 2028 listed: parent 1130, group 1142, session 1142 |
| `observe-forks-late` | 5 s after the last control | both not listed | 2020 and 2028 listed, unchanged |
| `observe-after-control-cleanup` | after the TERM | both not listed | both not listed |

Each observation read the PIDs that the control wrote. Each row is a `ps -p`
listing of that PID with its exit status. The checker counts a process as
absent only from a valid observation where `ps` exited 1.

The full process tables before and after `cli-sleep` and `raw-sleep` list no
`sleep 120`.

In the r5 case, `r5-observe-timeout` listed the timeout child 2158 under parent
1130. `r5-observe-cancel` listed 2158 and the cancel child 2168. The r5 cleanup
then removed both. `r5-observe-after-cleanup` listed neither. The other r5 calls
ended as in the original run. `nonzero` exited 23, `bad-install` exited 1, the
missing artifact failed with `DOWNLOAD_FAILED`, and the rest exited 0.

### Bound violations

| Bound | Outcome |
| --- | --- |
| host time at most the guest timeout plus 20 s | met, at most 2.115 s |
| overhead at most 4 s | met, at most 0.115 s |
| kill, drain and reap within 2 s | violated twice: both fork children listed 5 s after their controls |
| each observation within 30 s | met, at most 18.096 s |
| guest cleanup within 30 s | met, at most 0.117 s |
| disposal within 45 s | met, 0.819 s |

`r5-observe-cancel` took 18.096 s. The agent serves one request at a time. It
held the observation until the 20 s guest timeout of the cancelled call ended.
The original run showed the same delay, 18.2 s.

### Causal limits

- The fork children survived on a real guest kernel under systemd. The container
  result of k5 therefore holds on the clone.
- The leaf ran one sample of each control. A single absent child would not prove
  that the handler cleans up.
- The agent was fresh at the start of the plan. The leaf did not measure how the
  survivors of many timeouts add up in a long-lived agent.
- The CLI and the raw path end in the same handler. The survival is a property of
  the handler, not of the Host CLI.
- The leaf did not test a disconnect. Leaf `linux-disconnect-availability-k6`
  owns that question.
- The `timed_out` field cannot prove cleanup. The leaf judged cleanup only from
  its own observations.

### Owned-resource audit of attempt 02

| Resource | Owner proof | Bound | Outcome |
| --- | --- | --- | --- |
| clone `k204-linux-9ecaf72af04d4ffaa6ffbc79e1b882d8` | name with the run ID, created by the controller | disposal 45 s | disposed in 0.819 s, absent from `tart list` and `vm list` after |
| fork children 2020 and 2028 | PID file of the control, command exactly `sleep 120` | cleanup 30 s | ended by TERM in 0.100 s, then not listed |
| r5 children 2158 and 2168 | PID file of the fixture | cleanup 30 s | removed by the r5 cleanup, then not listed |
| final cleanup | the four PID files | cleanup 26 s | each process already gone, `cleanup confirmed` |
| guest directories `/tmp/taw-k204` and `/tmp/vmfit-k15` | created by the plan | disposal | removed with the clone |

The cleanup acted only on a PID whose command was exactly `sleep 120`. It sent
TERM first and KILL only after 2 s. It sent no KILL.

### Attempts

- `live-01`: the CLI controls ran, and `cli-fork` already showed failure B. The
  raw client could not reach the guest. Each raw call failed with
  `No route to host` (errno 65). The partial file of `raw-fork` therefore did not
  exist, and its download stopped the plan. The controller cleaned up child 2007
  with TERM and disposed the clone. The 28 later calls did not run.
- The cause of `live-01` is on the host. macOS Local Network privacy refuses a
  connection to `192.168.64.x` from an executable that is not a platform binary.
  The raw client ran in the virtual environment Python, which is not one.
  `docs/research/macos-local-network-platform-binary-exemption.md` records the
  rule. The refreeze sends each raw request through `/usr/bin/curl`, a platform
  binary. The raw record schema moved to `k204-raw/2`.
- `live-02`: complete and sound. The probe verdict is True, and the checker exits 0.

The leaf did not commit the refreeze before `live-02`.
`live-02-outer/source-dirty.diff` holds the uncommitted tools of that attempt,
relative to commit `cb1fc475af57ccbfc6761555dfd7d787ff49975e`.

### Provenance

- Host CLI `testanyware 2.8.0`, tart 2.41.1.
- The installed Linux agent bundle, the checkout `agents/linux` and the guest
  agent have equal hashes for all 9 modules.
- The tool digests before and after `live-02` are equal.
- `live-02/inventory.json` records the fixture hashes, the bounds and the frozen
  plan. The fixture is the retained r5 copy in `fixture-r5`.

### Evidence

`experiments/exec-reliability/evidence/linux-timeout-live-k204/` holds
`fixture-r5`, `live-01`, `live-01-outer`, `live-02` and `live-02-outer`. Each
outer directory holds the inventories, the identities, `check.json` and the exit
status of each step.

### Live invocation

```sh
task diagnostic:linux-timeout-live OUT=/path/to/new/out
task diagnostic:linux-timeout-live-check RUN=/path/to/new/out/<run-id>
task diagnostic:linux-timeout-live-tools
```

The run needs the golden image, tart and the Host CLI on `PATH`. It boots one
clone and disposes it. The run exits 0 only when every call ends as planned and
the checker exits 0. A reproduced failure B does not fail the run.

## Run against the lifecycle owner (leaf k217)

Leaf k217 gave the Linux agent its lifecycle owner and the typed exec record.
On 2026-11-18 the leaf ran the unchanged matrix once against that agent. The
run is `experiments/exec-reliability/evidence/linux-timeout-lifecycle-k217/run-01`.
Its run directory is `d57b537cd9914b629f13bcfa65913ddb`, and its tag is
`960270925275`. It found no leftover subject.

| Step | Exit | Result |
| --- | --- | --- |
| `diagnostic:linux-timeout-lifecycle` | 0 | Verdict true. No ledger entry and no residual. |
| `diagnostic:linux-timeout-lifecycle-check` | 0 | The evidence is sound. |
| `diagnostic:linux-timeout-lifecycle-old-path` | 201 | Seven frozen defects are missing. |

The old-path detector now fails. These defects are missing:

- `survivor-at-bound` for `issue-fork-wait`, in the process, traced, HTTP,
  CLI text and CLI JSON seams. The owner ends the whole group before it
  replies.
- `term-skipped` for traced `deadline` and traced `issue-sleep`. The owner
  sends SIGTERM to the group first.

The other defects stay, on purpose. They are properties of the legacy fields,
and the legacy fields keep the values of the old agent. The typed record beside
them carries the exact bytes and the outcome. The matrix does not read the
record.

Every HTTP, CLI text and CLI JSON receipt of the run equals its receipt in
`run-03`. For HTTP, the legacy fields are equal, and the reply has no
`timedOut`. For CLI JSON, the envelope is equal apart from the `command` text,
because the deadline commands differ in their sleep length. So the unchanged
Host CLI prints the same output and returns the same exit code.

The files `check.stdout`, `check.exit`, `old-path.stdout`, `old-path.stderr`
and `old-path.exit` keep each checker output and its exit status.
