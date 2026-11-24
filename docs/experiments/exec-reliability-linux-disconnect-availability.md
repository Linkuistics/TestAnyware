# Linux agent availability after a client disconnect (leaf linux-disconnect-availability-k6)

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

This report asks one question for issue #14 on Linux. When the command CLI stops
during a long `exec`, which boundary blocks the next request? The report also
measures whether the known child of the stopped command survives. It does not choose
cancel-on-disconnect, continued work, a server replacement or durable jobs. It makes
no claim about Windows.

## Result

The live run reproduced failure C. The CLI received SIGTERM at 2.007 s and exited at
2.010 s. The next request was the known-child inspection, which started at once. It
answered at 20.077 s, after 18.067 s. Issue #14 recorded 18.066 s for the same
failure. Health, an ordinary command and the partial-file download then answered in
0.010 s to 0.019 s.

The blocking boundary is serial request handling in the Linux agent.
`agents/linux/testanyware_agent/server.py` serves with a plain `HTTPServer`, which
handles one request at a time. The `/exec` handler blocks in `subprocess.run` until the
guest deadline. A request that arrives during that time waits in the kernel accept
queue. The disconnect does not cause the block, and it does not end it.

The container controls separate the causes. With no disconnect, a request during
execution also waited 18.021 s. A client close, a client reset and a client
half-close each gave the same wait, from 18.016 s to 18.041 s.

A 5 s host deadline
did not free the agent. The agent later read the abandoned request and wrote to a
closed socket. The threaded server, an experimental control only, answered during
execution in 0.002 s. Its handler still ran to the 20 s deadline.

In every container case the handler ran to its deadline. On the clone the deadline
reply shows the same end in `control` and `concurrent`. In `term` the evidence shows
only that the handler was gone by 20.077 s. The `sleep 120` child survived the handler
in every case. On the clone, `systemd --user` adopted the child. The owned cleanup
ended each child, and the controller disposed the clone.

Integration k208 corrected the live handler rule, the live cleanup, the live
provenance record and the replay path. The section *Integration k208* states each
correction and its limits.

## Oracle

The checker `experiments/exec-reliability/linux_k6_check.py` holds the oracle. It was
frozen before the measured runs, apart from correction r2. The section *Live run*
states that correction.

A follow-up request is `responsive` when three conditions hold:

1. It ends with its expected outcome inside its bound.
2. It does not span the end of the original handler. A spanning request began before
   the last sighting of the shell alive. It answered after the first sighting of the
   shell gone.
3. Its latency is at most the latency of the same request in the control, plus 5 s.

A request with the expected outcome that fails condition 2 or 3 is `blocked`. A
blocked request is a defect even when it ends inside its bound. A request that ends
with another outcome, or exceeds its bound, is `failed`. The control requests run
after the handler ends, so they are the `baseline`.

The checker classifies the handler, the child, the cleanup and the disposal
separately. A blocked channel does not decide them, and they do not decide the
channel.

The serial agent cannot answer an inspection while the handler runs. So the live run
has no sighting of the shell alive, and condition 2 cannot be evaluated. Condition 3
decides the live channel outcome.

The first live rule placed the handler end at the first sighting of the shell gone,
with the basis `queue-release`. Integration k208 withdrew that basis. A late answer
proves only that the shell was gone by that time. The corrected rule has three bases:

- `bracket`: a sighting of the shell alive and a sighting of it gone.
- `deadline-reply`: no sighting alive, outside `term`, and the expected Linux deadline
  reply. The agent writes that reply only after its deadline.
- `upper-bound`: any other case. An upper end before the deadline window is
  `cancelled`. Any other upper end is `gone-by`, with no class for the end itself.

In the container, the first queued request ended between 0.047 s before and 0.004 s
after the last shell sample. That supports the queue-release hypothesis for the same
agent sources. It does not bound the delay of the app and the transport on the clone.

## Container controls (`container-01`)

The controls ran in one fresh container of the pinned k5 image
`python@sha256:dddfd7e07f9d15aeeca61529320492139d21cac7f0070c00609243e51e4e0016`.
The container had no network and ran as uid 1000. Each case started a fresh,
unchanged agent from the checkout sources and sent the same fork/wait payload with
a 20 s guest timeout. At 2 s the case acted on the original connection. Then it sent
health, an ordinary command and a download of the partial file. A watcher sampled
the processes and the agent sockets every 50 ms.

| case | action at 2 s | original reply | health | ordinary, download | handler end |
|---|---|---|---|---|---|
| `serial-control` | none, follow-ups after the reply | 200 at 20.025 s | baseline 0.001 s | baseline 0.003 s, 0.001 s | 20.029 s |
| `serial-concurrent` | keep the connection | 200 at 20.022 s | blocked 18.021 s | responsive | 20.069 s |
| `serial-close` | close | none | blocked 18.033 s | responsive | 20.042 s |
| `serial-reset` | reset (linger 0) | none | blocked 18.041 s | responsive | 20.040 s |
| `serial-half-close` | shut down writes | 200 at 20.027 s | blocked 18.016 s | responsive | 20.052 s |
| `host-deadline` | health with a 5 s deadline | 200 | failed at 5.006 s, then blocked 13.008 s | responsive | 20.045 s |
| `threaded-reset` | reset, threaded server | none | responsive 0.002 s | responsive | 20.053 s |

The handler end is the first sample without the payload shell. The previous sample is
at most 0.06 s earlier. Every follow-up after the queued one answered in at most
0.003 s.

The socket rows show where each blocked request waited. The listening socket held one
pending connection from about 2.01 s. The accepted row held 66 to 67 unread request
bytes until the handler ended. The agent then read the request and answered it.

The disconnect changed only the original connection:

- After a close, the agent end of the original connection stayed in `CLOSE_WAIT`
  with 1 unread byte. When the deadline came, the agent tried to write its reply and
  logged `BrokenPipeError: [Errno 32] Broken pipe`. The error handler then tried to
  send a 500 reply and logged the same error.
- After a reset, the original row disappeared. The agent logged one `BrokenPipeError`
  at the deadline.
- After a half-close, the agent still delivered the deadline reply.
- After the host deadline, the abandoned health connection stayed in the accept
  queue. The listening socket then held two pending connections. The agent read the
  abandoned request at 20.015 s and logged `BrokenPipeError` when it answered.

The threaded case is labelled `experimental: threading-server`. It shows that a
concurrent server removes the queue wait. It is not cancellation evidence, because
its handler ran to the deadline.

In each case the `sleep` child was alive after the handler ended, with parent pid 1.
The cleanup checked the child pid and its start time, and killed that process only.
No owned process was left after any case, and the final sweep found none.

## Live run (`live-02`)

The live run used one fresh, unchanged clone of `testanyware-golden-linux-24.04`. It
used the installed CLI, testanyware 2.8.0, and tart 2.41.1. The agent in the clone is
the installed bundle, and its sources are equal to the checkout sources. The burst
script is the unchanged k13 script `disconnect_k13_burst.py`.

| mode | original | inspect | health | ordinary | download | handler end |
|---|---|---|---|---|---|---|
| `control` | exit 0 at 20.039 s | baseline 0.058 s | baseline 0.011 s | baseline 0.010 s | baseline 0.009 s | 20.039 s |
| `concurrent` | exit 0 at 20.043 s | blocked 18.079 s | responsive 0.023 s | responsive 0.010 s | responsive 0.009 s | 20.043 s |
| `term` | SIGTERM at 2.007 s, exit -15 at 2.010 s | blocked 18.067 s | responsive 0.019 s | responsive 0.011 s | responsive 0.010 s | gone by 20.077 s |

The handler end in `control` and `concurrent` is the exit of the original CLI, after
the deadline reply. In `term` no evidence bounds the end from below. The answer of the
blocked inspection at 20.077 s is only an upper bound. The first check reported
`deadline` for `term` by the withdrawn `queue-release` basis. The download returned the
8 bytes `partial\n` in each mode.

The deadline reply on Linux does not set `timed_out`. The CLI printed
`"exit_code": -1`, `"stderr": "Process timed out"` and `"timed_out": false`. The macOS
agent reports the same deadline with `timed_out` true. This report records the
difference as a finding.

### Children

The inspection at 18.01 s ran in the queue. It answered at 20.095 s, after the
handler ended. In `term` it listed the child `sleep 120` as pid 2194 with parent pid
1130. Pid 1130 is `/usr/lib/systemd/systemd --user`, a subreaper, not PID 1. The
inspection at 30.0 s showed the same child and parent. Each mode showed the same
adoption.

### Correction r2 and the failed sample `live-01`

The first live attempt `live-01` ran with the checker as first frozen. Its controller
verdict was True, and its clone was disposed. Its check exited 1, so it is retained as
a failed sample. Its raw records have the same shape as `live-02`. In `term` the
inspection waited 18.060 s.

Two k13 rules did not hold on Linux:

- The k13 rule required `timed_out` true in the control reply. The Linux reply sets it
  false.
- The serial agent answers the first inspection only after the handler ends. That
  inspection listed the subreaper at the recorded shell pid. The k13 rule took it for
  the shell and classified the handler as overran.

Correction r2 changed the live rule before `live-02`. It judges the control reply by
the Linux shape and records `timed_out` as a finding.

The rule counts a shell sighting as
alive only when its command names the mode directory. It records the parent of the
child. Neither change depends on a wanted outcome. A post hoc judgement of `live-01`
with r2 gives the `live-02` classes. It is not a second measured run.

## Integration k208

The review `linux-disconnect-availability-k207` reported four findings in
`docs/experiments/exec-reliability-linux-disconnect-availability-review-k207.md`.
Integration k208 found each finding valid. No new measurement ran.

| finding | correction | limit that stays |
|---|---|---|
| A queued inspection gave the live handler class from its upper end alone. | The checker reports `gone-by` for `term`, as *Oracle* states. | The `term` handler end on the clone is unmeasured. |
| The live cleanup checked the child command once, then signalled the numeric PID twice. A reused PID could receive the signal. | `linux_k6.pidfd_cleanup` opens a pidfd first. It binds the PID to `sleep 120` and to a start time near the mtime of `child.pid`. It sends TERM and KILL through the pidfd. | The retained live cleanups used the numeric-PID cleanup. They ended the expected PIDs, but they cannot exclude a wrong-process signal. |
| The live provenance omitted the app identity and the after-state of most installed agent modules. | The live plan hashes the app executable, its `Info.plist` and every installed agent module before and after. The runner records the app signature and paired digests. | The retained live runs lack those records. A digest taken now cannot stand for their after-state. |
| Both checker entry points read the run directory at its recorded absolute path. | The checker finds the run directory inside OUT by its base name. A RESULT name lets a re-judgement keep the retained `check.json`. | None found. |

The new cleanup ran in the pinned k5 image as a tooling control, outside any measured
run. A true child ended by TERM. A child that ignored TERM ended by KILL. A `sleep 120`
under a PID file 60 s older stayed alive as `other`. So did a `sleep 121`. An
unreadable PID gave `cleanup uncertain` and exit 1.

The identity check has two residual limits. A process that runs `sleep 120` and starts inside the window under
the recorded PID passes it. A guest clock step can turn the true child into `other`,
which leaves it to disposal.

The corrected checker re-judged the three retained OUT directories. Each result is
`check-k208.json` beside the retained `check.json`. Each re-judgement exited 0. The
record is in `rejudge-k208/`, with the source commit, the dirty diff, the tool digests
and the digests of the 114 retained files. Each digest list was equal before and after.

- `container-01` gave the same classes as its first check.
- `live-02` changed only the `term` handler, from `deadline` to `gone-by`.
- `live-01` gave the `live-02` classes. It stays a failed sample, because its frozen
  check exited 1. The re-judgement is not a second measured run.

### What consumers may rely on

- On Linux, serial request handling blocks every request during a long `exec`. The
  container controls show the queue, and the clone shows a wait of the same length.
- A close, a reset, a half-close or a client deadline does not free the agent.
- In each case the fork/wait child survived the handler. On the clone, `systemd --user`
  adopted it.
- The Linux deadline reply does not set `timed_out`.

### What remains unproved

- When the handler ended on the clone in `term`, and so whether a SIGTERM of the CLI
  changes the handler end. The container shows no change for the same sources.
- That the retained live cleanups signalled only the payload children.
- The app identity and the installed agent after-state of `live-01` and `live-02`.

A later Linux live run that needs these facts must use the corrected tooling. Its
evidence must keep a new identity, apart from `live-01` and `live-02`. No such run is
planned in this grove. The design leaf `lifecycle-evidence-k8` consumes this report
and must treat these items as open.

## Causal limits

- One container run and one live run count as evidence. The result is a sample, not
  a rate.
- The live run cannot see the guest accept queue. The container shows it for the
  same agent sources. The live wait has the same length and the same end.
- The live run does not show when the app relay closed the guest connection after the
  SIGTERM. The container shows that close, reset and half-close give the same wait.
- On the clone, the original CLI exit came 20.039 s after the host start in
  `control`. In `live-01` it came at 19.943 s, before the 20 s guest timeout. Clock
  rate in the guest is one possible cause. No receipt shows it.
- The container runs Python 3.12 from the pinned image, not the guest Python. The
  controls used the same agent sources.
- No run patched the installed CLI, the app or the golden image. No clone was
  instrumented.
- The retained live provenance has no app signature, no app digest and digests of the
  installed agent from before the run only. The live plan hashed only `server.py` of
  the installed agent before and after.

## Design inputs

- The blocked channel comes from serial handling. A design that only cancels on
  disconnect still blocks every request during an `exec` that has no disconnect.
- A client deadline does not free the agent. An abandoned request stays in the queue,
  and the agent processes it after the handler ends.
- A concurrent server removes the wait in the container. It does not end the handler
  or its child.
- The handler kills only the shell at its deadline. The fork/wait child survives it.
  On the clone a `systemd --user` subreaper adopts the child, so the child's parent is
  not PID 1.
- The Linux deadline reply does not set `timed_out`. A client cannot use that flag to
  detect a Linux deadline.

## Resource audit

| item | `container-01` | `live-02` |
|---|---|---|
| acquire | 0.092 s, exit 0 | 19.437 s, exit 0 |
| `health-before`, `health-after` | none | 0.078 s, 0.074 s, exit 0 |
| owned cleanup | 0.081 s within 20 s, `docker stop` | 0.068 s within 26 s, `cleanup confirmed` |
| disposal | 0.072 s within 40 s | 0.711 s within 41 s |
| post-disposal inspection | 0.071 s, exit 1 with `[]` | 0.146 s, exit 0 |
| resource after the run | absent from `containers-after.jsonl` | absent from `vm-after.json` and `tart-after.json` |
| children | each killed by the case cleanup, none left | pids 1837, 2102 and 2194 ended by SIGTERM |
| tooling digests | equal before and after | equal before and after |
| controller residuals | none | none |

`live-01` also disposed its clone. Its cleanup ended pids 1802, 2095 and 2189 by SIGTERM in
0.102 s, and its disposal took 0.740 s.

## Accounting

There was one container run, `container-01`, and two live attempts. `live-01` failed
its check and is retained. `live-02` is the measured live run after correction r2.
No run had a retry. Development trials ran in the scratchpad and are not evidence.

Integration k208 added only re-judgements, in `check-k208.json` files and in
`rejudge-k208/`. It changed no retained file.

The source commit of each run is `b414a4f7f46eb5516b1f5e457225f13df0435aac`. Each
`-outer` directory holds the dirty diff, the tool digests, the host identity and the
agent hashes.

The first container trial matched its process sweep against every command line that
held the run tag. The container init held the tag, so the sweep killed it. That trial
was not evidence. The measured seam matches only the payload, the payload shell and
the case agents.

## Commands

```
mise exec -- task diagnostic:linux-disconnect-tools
mise exec -- task diagnostic:linux-disconnect-container OUT=<new dir>
mise exec -- task diagnostic:linux-disconnect-live OUT=<new dir>
mise exec -- task diagnostic:linux-disconnect-check MODE=container OUT=<dir>
mise exec -- task diagnostic:linux-disconnect-check MODE=live OUT=<dir>
mise exec -- task diagnostic:linux-disconnect-check MODE=live OUT=<dir> RESULT=<new name>
```

A RESULT name keeps the retained `check.json` of a retained OUT.

The evidence is in
`experiments/exec-reliability/evidence/linux-disconnect-availability-k6/`.
