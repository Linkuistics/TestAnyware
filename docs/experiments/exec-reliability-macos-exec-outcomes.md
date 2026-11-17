# macOS exec outcomes

Leaf `macos-exec-outcomes-k12` freezes a finite matrix of exec outcomes on the
macOS process path. It records what each layer reports for each outcome. The
layers are the process runner, the agent HTTP response and the Host CLI receipt.
The leaf runs every case once on the host. It uses no clone.

The leaf makes no repair and defines no terminal record. Leaf `lifecycle-evidence-k8` receives this
evidence. The completion wait race stays with k3, and inspection stays with k4.

## Summary

- The reference seam reports every case faithfully, so the host can observe
  each outcome exactly.
- The unchanged runner loses facts in six ways. It trims each trailing line
  feed. It returns an empty stream for bytes that are not UTF-8. It reports a
  signal death as an exit status with the signal number. It reports a deadline
  as exit status -1. It writes `Process timed out after 2s` into stderr. Its
  grace after TERM is 3 s, not 2 s.
- The agent cannot report a spawn failure. A thrown spawn error becomes an HTTP
  500 with an empty body. The CLI then reports `AGENT_ERROR_UNKNOWN`.
- The CLI text mode exits 255 on a deadline and has no timeout flag.
- Three CLI JSON calls that ended by themselves came back as 10 s timeouts. This
  matches the completed-then-timed-out receipt of k3.
- Leaf k201 repaired three review findings in the tooling. A repeat run showed
  the same defects. None of its CLI JSON calls came back as a timeout.

## Matrix

Each case is a `bash -c` command. The timeout is 10 s, except on the two
deadline cases, which use 2 s. Each seam has its own sleep token, such as
`sleep 30.31`, so the audit can find each process by its exact command. Since
k201, the token also ends with a 12-digit run tag, such as
`sleep 30.31123456789012`. The tag identifies the process as a process of this
run.

| Case | Command | Faithful outcome |
| --- | --- | --- |
| `oracle-text` | `printf 'Explain this change.'; printf 'separate stderr\n' >&2` | exit 0, the oracle bytes |
| `binary` | `printf '\000\377\200A\n'` | exit 0, stdout `00ff80410a` |
| `exit-23-partial` | `printf 'partial out\n'; printf 'partial err\n' >&2; exit 23` | exit 23, both lines with LF |
| `signal-kill` | `printf 'before signal\n'; kill -KILL $$` | signal 9, stdout with LF |
| `exit-9` | `printf 'before exit\n'; exit 9` | exit 9, stdout with LF |
| `deadline` | `printf 'before deadline\n'; sleep 30.<seam>1<tag>` | deadline, TERM ends it |
| `deadline-term-ignored` | `trap '' TERM; printf 'ignoring term\n'; sleep 30.<seam>2<tag>` | deadline, KILL ends it |
| `spawn-failure` | `printf 'never runs\n'` under `RLIMIT_NPROC` of 1 | spawn error, errno 35 |

The oracle stdout is `4578706c61696e2074686973206368616e67652e` (20 bytes). The
oracle stderr is `7365706172617465207374646572720a` (16 bytes). The `exit-9`
case is the control for `signal-kill`. Without the termination reason, the two
cases give the same report.

## Seams

| Seam | What it runs | What it records |
| --- | --- | --- |
| `reference` | Python `bash -c`, temporary-file capture, `os.wait4` | raw wait status, end, code, phase times |
| `process` | the unchanged `ProcessRunner.runShell` in a probe | the returned `ProcessRunResult` as hex |
| `traced` | the same probe built with `-DEXIT_TRACE` | the result, plus the kernel exit status and each phase mark |
| `http` | a loopback copy of the agent, `POST /exec` | HTTP status, headers and body bytes |
| `cli-text` | `testanyware exec CMD --vm VM --timeout T` | host exit status, stdout and stderr bytes |
| `cli-json` | the same command with `--json` | host exit status, stdout and stderr bytes |

The loopback agent copies `agents/macos` without its tests. Only its `main.swift`
changes. That file binds `127.0.0.1` and does not start the notification task.
`AgentServer.swift` and `ProcessRunner.swift` stay byte-identical, and the build
checks this.

The CLI talks to the agent through a test-mode app on a private
socket under `/tmp`. Two VM specs point at two agents. The second agent runs
under `RLIMIT_NPROC` of 1, so its next spawn fails with EAGAIN.

The runner controls spawn failure at the process seam. It is not a natural fault. The
limit applies to the probe or to the agent, never to the controller.

## Results

Run `run-01` ran once, with no retry. The controller verdict is true. The
checker reports no problem, and the old-path detector exits 0. Each table gives
the bytes as hex. The CLI text mode has no `timed_out` field, so the cell is
empty.

#### `oracle-text`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | exit | 0 (raw 0) | false | `4578706c61696e2074686973206368616e67652e` | `7365706172617465207374646572720a` | 0.004 | none |
| process | exit | 0 | false | `4578706c61696e2074686973206368616e67652e` | `736570617261746520737464657272` | 0.270 | `trailing-lf-trimmed` |
| traced | exit | 0 | false | `4578706c61696e2074686973206368616e67652e` | `736570617261746520737464657272` | 0.263 | `trailing-lf-trimmed` |
| http | exit | 0 | false | `4578706c61696e2074686973206368616e67652e` | `736570617261746520737464657272` | 0.077 | `trailing-lf-trimmed` |
| cli-text | exit | 0 |  | `4578706c61696e2074686973206368616e67652e` | `7365706172617465207374646572720a` | 1.425 | none |
| cli-json | exit | 0 | false | `4578706c61696e2074686973206368616e67652e` | `736570617261746520737464657272` | 0.113 | `trailing-lf-trimmed` |

#### `binary`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | exit | 0 (raw 0) | false | `00ff80410a` | `—` | 0.004 | none |
| process | exit | 0 | false | `—` | `—` | 0.013 | `non-utf8-dropped` |
| traced | exit | 0 | false | `—` | `—` | 0.018 | `non-utf8-dropped` |
| http | exit | 0 | false | `—` | `—` | 0.073 | `non-utf8-dropped` |
| cli-text | exit | 0 |  | `—` | `—` | 0.094 | `non-utf8-dropped` |
| cli-json | exit | -1 | true | `—` | `50726f636573732074696d6564206f757420616674657220313073` | 10.072 | `completed-reported-timeout`, `non-utf8-dropped`, `synthetic-exit-code`, `timeout-text-in-stderr` |

#### `exit-23-partial`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | exit | 23 (raw 5888) | false | `7061727469616c206f75740a` | `7061727469616c206572720a` | 0.004 | none |
| process | exit | 23 | false | `7061727469616c206f7574` | `7061727469616c20657272` | 0.013 | `trailing-lf-trimmed` |
| traced | exit | 23 | false | `7061727469616c206f7574` | `7061727469616c20657272` | 0.017 | `trailing-lf-trimmed` |
| http | exit | 23 | false | `7061727469616c206f7574` | `7061727469616c20657272` | 0.075 | `trailing-lf-trimmed` |
| cli-text | exit | 23 |  | `7061727469616c206f7574` | `7061727469616c206572720a` | 0.035 | `trailing-lf-trimmed` |
| cli-json | exit | -1 | true | `7061727469616c206f7574` | `7061727469616c206572720a5b50726f636573732074696d6564206f7574206166746572203130735d` | 10.098 | `completed-reported-timeout`, `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |

#### `signal-kill`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | signal | 9 (raw 9) | false | `6265666f7265207369676e616c0a` | `—` | 0.004 | none |
| process | exit | 9 | false | `6265666f7265207369676e616c` | `—` | 0.013 | `signal-reported-as-exit`, `trailing-lf-trimmed` |
| traced | exit | 9 | false | `6265666f7265207369676e616c` | `—` | 0.016 | `signal-reported-as-exit`, `trailing-lf-trimmed` |
| http | exit | 9 | false | `6265666f7265207369676e616c` | `—` | 0.070 | `signal-reported-as-exit`, `trailing-lf-trimmed` |
| cli-text | exit | 9 |  | `6265666f7265207369676e616c` | `—` | 0.092 | `signal-reported-as-exit`, `trailing-lf-trimmed` |
| cli-json | exit | -1 | true | `6265666f7265207369676e616c` | `50726f636573732074696d6564206f757420616674657220313073` | 10.051 | `completed-reported-timeout`, `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |

#### `exit-9`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | exit | 9 (raw 2304) | false | `6265666f726520657869740a` | `—` | 0.004 | none |
| process | exit | 9 | false | `6265666f72652065786974` | `—` | 0.013 | `trailing-lf-trimmed` |
| traced | exit | 9 | false | `6265666f72652065786974` | `—` | 0.082 | `trailing-lf-trimmed` |
| http | exit | 9 | false | `6265666f72652065786974` | `—` | 0.071 | `trailing-lf-trimmed` |
| cli-text | exit | 9 |  | `6265666f72652065786974` | `—` | 0.096 | `trailing-lf-trimmed` |
| cli-json | exit | 9 | false | `6265666f72652065786974` | `—` | 0.028 | `trailing-lf-trimmed` |

#### `deadline`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | deadline-signal | 15 (raw 15) | true | `6265666f726520646561646c696e650a` | `—` | 2.004 | none |
| process | exit | -1 | true | `6265666f726520646561646c696e65` | `50726f636573732074696d6564206f7574206166746572203273` | 2.150 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| traced | exit | -1 | true | `6265666f726520646561646c696e65` | `50726f636573732074696d6564206f7574206166746572203273` | 2.127 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| http | exit | -1 | true | `6265666f726520646561646c696e65` | `50726f636573732074696d6564206f7574206166746572203273` | 2.266 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| cli-text | exit | 255 |  | `6265666f726520646561646c696e65` | `50726f636573732074696d6564206f75742061667465722032730a` | 2.232 | `deadline-not-flagged`, `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| cli-json | exit | -1 | true | `6265666f726520646561646c696e65` | `50726f636573732074696d6564206f7574206166746572203273` | 5.243 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |

#### `deadline-term-ignored`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | deadline-signal | 9 (raw 9) | true | `69676e6f72696e67207465726d0a` | `—` | 4.001 | none |
| process | exit | -1 | true | `69676e6f72696e67207465726d` | `50726f636573732074696d6564206f7574206166746572203273` | 5.190 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| traced | exit | -1 | true | `69676e6f72696e67207465726d` | `50726f636573732074696d6564206f7574206166746572203273` | 5.134 | `grace-exceeded`, `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| http | exit | -1 | true | `69676e6f72696e67207465726d` | `50726f636573732074696d6564206f7574206166746572203273` | 5.190 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| cli-text | exit | 255 |  | `69676e6f72696e67207465726d` | `50726f636573732074696d6564206f75742061667465722032730a` | 5.249 | `deadline-not-flagged`, `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |
| cli-json | exit | -1 | true | `69676e6f72696e67207465726d` | `50726f636573732074696d6564206f7574206166746572203273` | 5.249 | `synthetic-exit-code`, `timeout-text-in-stderr`, `trailing-lf-trimmed` |

#### `spawn-failure`

| Seam | End | Code | `timed_out` | stdout hex | stderr hex | Seconds | Defects |
| --- | --- | --- | --- | --- | --- | --- | --- |
| reference | spawn-error | 35 |  | `—` | `—` | 0.002 | none |
| process | spawn-error | 35 |  | `—` | `—` | 3.051 | none |
| traced | spawn-error | 35 |  | `—` | `—` | 3.081 | none |
| http | http-500 | None |  | `—` | `—` | 3.036 | `spawn-failure-unmapped` |
| cli-text | cli-error | 1 |  | `—` | `—` | 3.079 | `spawn-failure-unmapped` |
| cli-json | cli-error | AGENT_ERROR_UNKNOWN |  | `—` | `—` | 3.094 | `spawn-failure-unmapped` |

### Phase times

The reference seam sends TERM to the process group at the deadline. It waits up
to 2 s, then sends KILL.

| Case | Deadline to TERM | Grace | KILL to reap |
| --- | --- | --- | --- |
| `deadline` (reference) | 0 ms | 2.5 ms, ended by TERM | none |
| `deadline-term-ignored` (reference) | 0 ms | 2.000 s, expired | 0.5 ms |

The traced runner gives these phase marks, in milliseconds from `begin`:

| Mark | `deadline` | `deadline-term-ignored` |
| --- | --- | --- |
| `wait-result` deadline, timed out | 2000.6 | 2002.6 |
| `descendants-end` (pgrep walk) | 2043.8 | 2044.8 |
| `terminate-sent` to bash only | 2043.8 | 2045.7 |
| `kernel-exit` bash | 2044.2, status 15 | 5054.6, status 9 |
| `wait-result` terminate | 2110.4, success | 5048.9, timed out |
| `kill-sent` | none | 5054.0 |
| `reap-return` | none | 5117.4 |
| `kernel-exit` sleep | 2044.2, status 15 | 5118.2, status 9 |
| `return` | 2112.1, exit -1 | 5119.6, exit -1 |

The runner waits 3.003 s after TERM, which exceeds the 2 s grace bound. The reap
after KILL takes 63 ms, within the 2 s bound. The descendant walk adds 43 ms
before TERM. The runner sends TERM to bash only. In the `deadline` case the sleep
child also exits on status 15 at the same instant. In the ignored case the sleep
child lives until the descendant kill at 5117.5 ms.

The traced `exit-9` case shows the raw kernel status 2304 and the Foundation
termination status 9. The runner then returns exit code 9 and discards the
termination reason. In `signal-kill` the same runner also returns 9.

### Spawn failure

| Seam | Report | Seconds |
| --- | --- | --- |
| `reference` | `posix_spawn` error, errno 35 | 0.002 |
| `process` | thrown `NSPOSIXErrorDomain` code 35, "Resource temporarily unavailable" | 3.051 |
| `traced` | the same error, with only `trace-open` and `begin` before it | 3.081 |
| `http` | HTTP 500 Internal Server Error, `Content-Length: 0` | 3.036 |
| `cli-text` | host exit 1, stderr `error: HTTP 500 Internal Server Error: ` | 3.079 |
| `cli-json` | host exit 1, `{"code": "AGENT_ERROR_UNKNOWN", "details": {"body": "", "http_status": 500}, ...}` | 3.094 |

The runner keeps the errno, but the agent drops it. `Process.run()` takes about
3 s to throw, while a direct `posix_spawn` fails in 2 ms. The evidence does not
show what Foundation does during those 3 s.

### Completed calls reported as timeouts

The CLI JSON seam ran after the HTTP and CLI text seams on the same normal agent.
Three of its cases came back as timeouts at the 10 s request timeout:

| Case | Seconds | Report |
| --- | --- | --- |
| `binary` | 10.072 | exit -1, `timed_out` true, stdout empty |
| `exit-23-partial` | 10.098 | exit -1, `timed_out` true, stderr `partial err\n[Process timed out after 10s]` |
| `signal-kill` | 10.051 | exit -1, `timed_out` true, stdout `before signal` |

Each command ends in milliseconds on the other seams. The streams hold the
command output, so the command ran and ended. The `deadline` case on this seam
took 5.243 s, against 2.266 s on the HTTP seam. The extra 3 s is consistent with
a terminate wait that also timed out.

This receipt matches the k3 mechanism in `ProcessRunner.waitForExit`. The seam has no trace, so this run does
not prove that mechanism. The checker names these cases
`completed-reported-timeout`.

## Defects of the old path

| Defect | Where | Effect on a consumer |
| --- | --- | --- |
| `trailing-lf-trimmed` | runner, every seam after it | the output differs from what the command wrote |
| `non-utf8-dropped` | runner | binary output becomes an empty string with exit 0 |
| `signal-reported-as-exit` | runner | `kill -KILL` and `exit 9` give the same report |
| `synthetic-exit-code` | runner (-1), CLI text (255) | a deadline looks like a process status |
| `timeout-text-in-stderr` | runner | the runner note mixes with the command stderr |
| `grace-exceeded` | runner (3 s wait) | the deadline phase takes 3 s longer than the bound |
| `spawn-failure-unmapped` | agent, CLI | a spawn error looks like an unknown agent error |
| `deadline-not-flagged` | CLI text | only the stderr text tells a timeout from exit 255 |
| `completed-reported-timeout` | runner wait (k3) | a finished command reports a timeout |

The CLI text mode prints stderr with a trailing line feed. So `oracle-text`
looks faithful in text mode, although the agent trimmed the byte.

## Causal limits

- The run is local. The guest OS, the guest build and the guest load are not
  measured. The agent here is a debug build on macOS 27.0.1 (26A434), arm64.
- Each case ran once. The three timeouts show that the receipt occurs. They do
  not give its rate, and they do not show why only the third seam got it.
- The spawn failure comes from `RLIMIT_NPROC`. Other spawn failures, such as a
  missing `/bin/bash`, can take another path.
- The traced probe changes timing. The `EXIT_TRACE` marks are evidence for the
  traced runner, not proof for the default runner.
- A timeout is never inferred from stderr. The checker reads only `timed_out`
  and the status fields.

## Design implications

These are inputs for `lifecycle-evidence-k8`. This leaf does not choose a
record format.

- A terminal record needs the termination reason next to the code. Otherwise
  signal 9 and exit 9 stay the same.
- A deadline needs its own end state. It does not need an exit code.
- Streams need bytes, or a byte-exact encoding such as base64. Text decoding and
  trimming lose facts.
- A runner note needs its own field, apart from the command stderr.
- A spawn failure needs its own end state with the errno, through the agent and
  the CLI.
- The grace and the kill wait need their own bounds, with times in the record.
- Any change to the CLI surface must stay compatible, because many consumers
  use only the CLI.

## Provenance

| Item | Value |
| --- | --- |
| Commit | `84eacabec5c7d36ada25194be884f7f795e3d6ae` |
| Host | macOS 27.0.1 (26A434), arm64, kernel 27.0.0 |
| Swift | Apple Swift 6.4.2 (swiftlang-6.4.2.1.9) |
| Cargo | 1.99.1 |
| Python | 3.12.15 |
| `ProcessRunner.swift` | `6a3c3846eb88ca939c54941da18ed6443632759279fd1b614d71c1af109d1d37` |
| `AgentServer.swift` | `5ba8f16c34dc922448d9d28163f050720e4110c74a6064d2f191828e98f6ba5f` |
| `ExitTrace.swift` | `079f0ad4620d7a1559c7ba5bcfee625b953850a70c4102780a47b5c5beba3676` |
| default probe binary | `6f68f29317b59b12072706b9f63f47b41c285ba63aa1ff0c37fa7e558cb57f79` |
| traced probe binary | `2710e0ffccd1bcf5fd30b44d769eaa381af82b40c76a7b7faecff5d790c4818e` |
| loopback agent binary | `1e657250f89402d42ece1f3f63be69306784926a72f46f9ef3eba494fe4de9a6` |
| CLI binary | `6be9abf7acb89ed90219f442f534481cb7886ad5a9b9382ff0ff9e7266284eda` |
| app binary | `6fcb0de0ea31b5571f41c941a1e9ca902f863c2db3c2c2996680fd7330ff30ed` |
| `cli-rs/Cargo.lock` | `96ee8c89fbc69a1e74f97af03df5e85528bae27ddacd1143ab568468607793e2` |
| `agents/macos/Package.resolved` | `9e4ea3550f73160ddd02d93296814252b86901ef24e1fed023cd5317c1785267` |
| `uv.lock` | `1c3ab314b597770f6568fa159164b8ac72af7d9c3c9fc74ef3d9cf47b73cfd89` |

The runner writes `inventory.json` before the first call. It holds the case
matrix, the bounds, the build commands and the plan. The source hashes were the
same before and after the run.

## Owned-resource audit

- The controller ran four calls: `reference` 6.2 s, `process` 10.9 s, `traced`
  11.0 s and `http-cli` 69.3 s. Each returned 0 within its budget.
- Each agent and the app became ready, then stopped on TERM with status 0. No
  process group member needed KILL.
- No case left a process with its sleep token. The final audit found no
  leftover.
- The build directory and the `/tmp` root of the app are gone.
- The run used no clone, no default socket, no installed app and no `pkill`.

## Evidence

`experiments/exec-reliability/evidence/macos-exec-outcomes-k12/run-01/` holds
`inventory.json`, the build logs, `summary.json` and `check.json`. The run
directory `b695d8e3a3d648ab9b0cf2e08ce5547d/` holds the plan, the controller
result and the stdout of each seam call.

## Review integration (k201)

Review leaf `macos-exec-outcomes-k200` raised three findings. Leaf
`macos-exec-outcomes-k201` checked each one against the source. The source
supports all three, and k201 repaired each one.

| Finding | Classification | Repair |
| --- | --- | --- |
| P1, cleanup ownership | real issue against the root brief | Each sleep token ends with a run tag. `sweep()` and `audit()` read the start time and command again before each kill. |
| P2, checker gate | real issue | The class `phase-marks-invalid` rejects absent or unordered phase marks. The `deadline` case expects signal 15, and `deadline-term-ignored` expects signal 9. |
| P2, CLI ceiling | real issue against the root brief | The CLI bound is the guest budget plus 20 s, with no extra margin. The checker compares each measured time with that ceiling. |

The run tag is the run identifier modulo 10^12, as 12 digits. The runner
passes it to each seam call and records it in `inventory.json`. An orphaned
process moves to `launchd`, so its ancestry cannot identify it. The exact
command with the tag identifies it instead.

A new test control found one more defect. A reference record without `reap_ns`
crashed the checker with `KeyError`. The checker now measures phases only after
it accepts the marks.

The stricter checker accepts a copy of k12 `run-01`, and the old-path detector
exits 0 on that copy. The recorded `run-01` stays unchanged.

Run `macos-exec-outcomes-k201/run-01` used the repaired tooling once, with no
retry. The run tag was `934359825428`. The results are:

- The controller verdict is true. The checker reports no problem, and the
  old-path detector exits 0. Every frozen defect is still present.
- The reference seam reports every case faithfully. The `deadline` case ends
  with signal 15 after 2.004 s. The `deadline-term-ignored` case ends with
  signal 9 after 4.002 s.
- The longest case took 5.23 s, on the CLI text seam. The ceiling is 22 s.
- No case left a residual, and the final audit found no leftover. The build
  directory is gone.
- The controller calls took 6.2 s, 10.8 s, 10.8 s and 37.7 s. The `http-cli`
  call took 69.3 s in k12. In this run, no completed CLI JSON call came back as
  a timeout.

The completed-then-timed-out receipt is therefore intermittent. Later work can
rely on k12 `run-01` to show that it occurs. It cannot rely on either run for
its frequency. The race stays with k3.

The run directory is
`experiments/exec-reliability/evidence/macos-exec-outcomes-k201/run-01/`.

## Regression invocation

```sh
task diagnostic:macos-exec-outcomes OUT="$PWD/experiments/exec-reliability/evidence/macos-exec-outcomes-k12/run-02"
task diagnostic:macos-exec-outcomes-check OUT="$PWD/experiments/exec-reliability/evidence/macos-exec-outcomes-k12/run-02"
task diagnostic:macos-exec-outcomes-old-path OUT="$PWD/experiments/exec-reliability/evidence/macos-exec-outcomes-k12/run-02"
task diagnostic:macos-exec-outcomes-tools
```

The check fails only on unsound evidence. The old-path detector passes only when
every frozen defect is present. After a repair, the detector fails, and its
`MISSING` lines name each defect that the repair removed.
