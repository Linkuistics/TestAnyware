# macOS agent availability after a client disconnect (leaf macos-disconnect-availability-k13)

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

This report asks one question for issue #14 on macOS. When the command CLI stops
during a long `exec`, does the agent stay available for other requests? The report
also measures whether the known child of the stopped command survives. It does not
choose a cancel-on-disconnect design.

## Result

On one fresh unchanged clone, the agent stayed responsive after the command CLI
received SIGTERM at 2 s. Each follow-up request answered in 0.011 s to 0.091 s. The
known-child inspection, health, an ordinary command and the immutable partial file
download all had their expected outcomes. The last one answered at 2.281 s, and the
original handler ended after 18.01 s.

The disconnect did not cancel the handler. The guest shell and its `sleep 120`
child stayed alive at 18.0 s. Both were gone at the 30.0 s inspection. That bracket
is consistent with the 20 s guest deadline. It does not prove the deadline was the
cause, because the bracket cannot separate the deadline from a later end before 30 s.

The host diagnostic gives the stronger causal result. The loopback agent uses the
unchanged handler sources. On it, a client close, a client reset and a client
half-close each left the handler running until its 20 s deadline. Follow-up requests
during the handler answered in at most 0.073 s.

Two calls returned complete output with a reported guest timeout. One was the host
`cli-concurrent` ordinary command, and one was the live owned cleanup. This report
records both as findings and does not explain them.

## Oracle

The oracle was frozen before any live run. A follow-up request is responsive when
all three conditions hold.

1. The request has its expected outcome.
2. The request did not begin before the handler end and answer after it.
3. The request latency is at most the latency of the same request in the reference
   mode plus 5 s.

The reference for `concurrent` is `control`. The reference for `term` is
`concurrent`, and then `control` if the `concurrent` latency is not sound. A request
that fails condition 2 or condition 3 is `blocked`, even when it answers later. A
request that fails condition 1 is `failed`. A failed or timed-out inspection makes
the child outcome `unknown`, and a failed download stays a failure.

A call with complete output and a reported guest timeout is
`completed-reported-timeout`. That class is a finding and never a sound reference
latency. The k12 report uses the same receipt name.

## Host diagnostic (`host-01`)

The host diagnostic builds the agent from `agents/macos` with a loopback entry
point. The `ProcessRunner` and `AgentServer` sources stay byte-identical. It then
runs the agent, the debug `testanyware-app` and the debug CLI on the host. Each of
seven cases runs the issue fork/wait payload with a 20 s timeout and acts at 2 s.

| case | action at 2 s | shell gone | child gone | health | ordinary |
|---|---|---|---|---|---|
| `http-control` | none | 20.068 s | 20.070 s | not sent | not sent |
| `http-concurrent` | requests | 20.175 s | 20.178 s | 0.006 s | 0.069 s |
| `http-close` | close socket | 20.150 s | 20.153 s | 0.004 s | 0.073 s |
| `http-reset` | RST | 20.141 s | 20.143 s | 0.002 s | 0.068 s |
| `http-half-close` | shut write | 20.184 s | 20.186 s | 0.002 s | 0.069 s |
| `cli-concurrent` | requests | 21.343 s | 21.281 s | 0.011 s | 10.065 s |
| `cli-term` | SIGTERM to CLI | 20.065 s | 20.067 s | 0.023 s | 0.084 s |

The checker judged every handler as `deadline` and every child as
`ended-with-handler`. Every HTTP follow-up was `responsive`. In `cli-term`, the CLI
exited with signal 15 at 2.020 s after one SIGTERM. The KILL step was not needed.

The host seam polls the shell and the child only after each follow-up returns. So
in host mode, oracle condition 2 cannot fail. A request held until the handler end
would get its answer stamp before the end stamp. The latency condition carries the
host verdict instead. Each HTTP follow-up answered in at most 0.073 s, and each
handler ended after 20 s.

The probe sent the `cli-concurrent` ordinary command at 2.063 s and answered at 12.128 s. Its
reply held `stdout` `ordinary-ok`, `exit_code` -1 and `timed_out` true. That is a
completed reported timeout. In the same case, the probe first saw the guest shell
at 1.186 s. In the other cases, it first saw the shell at about 0.06 s.

Because the `cli-concurrent` ordinary latency is not sound, the `cli-term` ordinary
command has no reference. The checker reports "cli-term: a follow-up outcome is
unknown", and the host check exits 1. That `unknown` is correct under the frozen
oracle. The `cli-term` ordinary command itself answered in 0.084 s with exit 0.

### Re-judgement

The first check of `host-01` stays in `host-01/check-01.json` (SHA-256
`1da69d92c6f848a8dfdf02d285071b35ed2b3d6ea003b9f702cfdcf353a4e95d`). It had two
problems, because the checker did not yet know the completed-reported-timeout class.
The checker was then revised, and `host-01/check.json` holds its judgement of the
same receipts. No call ran again. The live run came after the revision.

## Live run (`live-01`)

The live run uses one clone of `testanyware-golden-macos-27` with the installed CLI
`testanyware 2.8.0`. The bundled agent has SHA-256
`22dd56cec883a9c471420172d0261617ed02a4fea218f949ee598a5311bab9f5`. The source
commit is `ad44194e203586e8a99f6b7ad01056d2d3d4f23a`, and
`live-01-outer/source-dirty.diff` holds the uncommitted tooling.

The plan is `health-before`, then three bursts, then `health-after`. Each burst runs
the issue payload in its own guest directory `/tmp/taw-k13/<mode>` with a 20 s guest
timeout.

| burst | original command | follow-ups begin |
|---|---|---|
| `control` | exit 0 at 20.216 s, `exit_code` -1, `timed_out` true | after the original reply |
| `concurrent` | exit 0 at 20.229 s, `exit_code` -1, `timed_out` true | at 2.006 s |
| `term` | SIGTERM at 2.009 s, CLI exit -15 at 2.010 s | at 2.010 s |

The follow-up latencies follow. Each follow-up had its expected outcome.

| request | `control` | `concurrent` | `term` |
|---|---|---|---|
| inspect | 0.091 s | 0.074 s | 0.091 s |
| health | 0.013 s | 0.013 s | 0.011 s |
| ordinary | 0.086 s | 0.077 s | 0.075 s |
| download | 0.032 s | 0.015 s | 0.013 s |
| inspect-after | 0.080 s | 0.077 s | 0.082 s |

Health reported `accessibility_status` granted and `reachable` true. Each download
wrote 8 bytes equal to `partial\n` (`7061727469616c0a`). The checker classes are
`baseline` for `control`, and `responsive` for every `concurrent` and `term`
request. The live check had no problem.

### Children

The inspections read the child PID from the payload file and the shell PID from the
child's parent. The raw tables follow for `term`.

| inspection | at | shell 918 | child 920 |
|---|---|---|---|
| inspect | 2.010 s | present, parent 426 | present, parent 918 |
| inspect-after | 2.200 s | present | present, parent 918 |
| inspect-mid | 18.006 s | present | present, parent 918 |
| inspect-late | 30.004 s | absent | absent |

The `concurrent` tables have the same shape with shell 760 and child 762. The
handler bracket is (18.01 s, 20.23 s] for `concurrent`, because its original reply
came at 20.229 s. For `term` the bracket is (18.01 s, 30.12 s]. The checker judged
both as `deadline` and both children as `ended-with-handler`.

These live labels are weaker than their names. The checker sets no upper limit on
the bracket end for `deadline`. In live mode, `ended-with-handler` means only that
the child was absent at the 30.0 s inspection. So the live receipts show that each
child ended between 18.01 s and 30.12 s, not that it ended with its handler.

The inspector checks a recorded PID with `kill -0` and `ps` and records only
the PID. A reused PID could therefore read as present. Both late inspections read
`absent`, and a reused PID cannot cause an absent reading. So these verdicts do not
depend on PID identity. A later live run with a present late reading needs a
process identity check before it counts as an overrun or a surviving child.

In `control`, the inspections ran after the original reply. They found child 601
absent and no shell record.

## Completed calls reported as timeouts

| run | call | reply | elapsed | usual |
|---|---|---|---|---|
| `host-01` | `cli-concurrent` ordinary | `ordinary-ok`, `exit_code` -1, `timed_out` true | 10.065 s | 0.07 s |
| `live-01` | owned cleanup | full output to `cleanup confirmed`, `exit_code` -1, `Process timed out after 9s` | 9.117 s | 0.02 s to 0.36 s |

The live cleanup began 28.3 s after the `term` SIGTERM, directly after a 0.072 s
`health-after`. Earlier cleanups in this experiment directory took 0.02 s to 0.36 s.
The controller accepted the cleanup because the CLI exited 0. The cleanup output
names each child PID file and then `gone`, so all three children were already gone.

The same shape appeared in development runs, which are not evidence. In those runs,
short commands answered fast before the first CLI TERM and sometimes took 10 s
afterwards. It also appeared once in the host `cli-concurrent` case. Before that case, that agent saw
three HTTP disconnects but no CLI TERM. So this report cannot tie the shape to the
disconnect. The k3 report and the k12 report record the same intermittent shape.

## Causal limits

- One clone and one run per mode. The result is a sample, not a rate.
- The live `term` bracket is (18.01 s, 30.12 s]. A handler that ended at 25 s for
  another reason would have the same receipts. The host `cli-term` case closes that
  gap for the loopback agent: the shell ended at 20.065 s.
- The live run does not show whether the app relay closed the guest connection at
  the CLI TERM. The host HTTP cases show that close, reset and half-close all leave
  the handler running.
- The completed-reported-timeout mechanism is open. Global dispatch queue starvation
  in `ProcessRunner.waitForExit` is one candidate, but no receipt shows it.
- The mutable-file case is out of scope. A corrupt mutable download goes to
  `download-consistency-k7`, and this run downloaded only the immutable partial file.

## Design inputs

- A cancel-on-disconnect design builds on an agent that does not cancel. The
  handler runs to its deadline after a close, a reset or a half-close.
- On the loopback agent, the deadline ends the shell and its known child. Each
  host child ended within 0.07 s of its shell. The live receipts show only that
  both live children ended between 18.01 s and 30.12 s.
- A disconnect alone did not make the agent unavailable in these samples. Follow-ups
  during the orphaned handler answered as fast as in the control.
- A client must not treat `timed_out` true as proof that the command did not finish.
  Two calls in this leaf printed their complete output and still reported a timeout.

## Resource audit

| item | result |
|---|---|
| clone acquire | 15.315 s, exit 0 |
| `health-before`, `health-after` | 0.084 s, 0.072 s, exit 0 |
| owned cleanup | 9.117 s within its 26 s bound, `cleanup confirmed` |
| disposal | 1.114 s within its 41 s bound, exit 0 |
| post-disposal inspection | 0.149 s, exit 0 |
| clone after the run | absent from `vm-after.json` |
| app PID | 53925 before and after |
| tooling digests | equal before and after |
| controller residuals | none |
| host build directory | removed, no leftover process |
| host servers | agent and app stopped by SIGTERM, root directory removed |

The cleanup took 9.117 s against the 30 s limit, and disposal took 1.114 s against
the 45 s limit.

## Accounting

There was one host run and one live run. Neither had a retry. Development trials
and dry runs used scratch directories and are not evidence. The host check exits 1
for the `unknown` above, and the live check exits 0.

The host run cleaned up with a substring match. It killed any process of this user
whose command held a run token or the build directory. Its sweeps and its audit were
empty, so it killed no process. Integration `macos-disconnect-availability-k203`
narrowed that match to the shapes that the run creates. These shapes are the payload
`sleep`, the payload shell and a binary inside the build directory.

The checker did
not change. A new host run uses the changed tools, so it is a new condition.

## Commands

```
mise exec -- task diagnostic:macos-disconnect-availability-tools
mise exec -- task diagnostic:macos-disconnect-availability-host OUT=<new dir>
mise exec -- task diagnostic:macos-disconnect-availability OUT=<new dir>
mise exec -- task diagnostic:macos-disconnect-availability-check MODE=live OUT=<dir>
```

The evidence is in
`experiments/exec-reliability/evidence/macos-disconnect-availability-k13/`.
