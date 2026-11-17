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
