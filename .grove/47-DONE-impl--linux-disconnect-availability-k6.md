# linux-disconnect-availability-k6


## Goal

Locate the Linux request/transport boundary that blocks observation after abrupt
CLI interruption, while independently accounting for surviving guest work.



## Context

- Issue #14 failure C: approximately 18.066 s for observation after interrupting
  a 20 s exec at 2 s. Retained raw diagnosis and the report/owned fixtures from
  `linux-timeout-lifecycle-k5`; classification alone proves no cancellation.
- Linux `run_server`, `AgentRequestHandler.do_POST`, `handle_exec` and download
  dispatch; Rust transport fixtures and CLI/app routing tests.

## Done when

- Freeze one fresh unchanged Linux clone's exact fork/wait payload with guest
  timeout 20 s. The independent controller sends SIGTERM to that command CLI at
  2 s, allows <=2 s, then SIGKILL with <=2 s more if needed. Retain actual host
  signals/status, phase timing and all available partial evidence.
- Immediately issue independent process observation, health and partial-file
  download requests, each bounded separately (ordinary 30 s, transfer 150 s).
  Record request acceptance/response and original-handler end. Compare a
  no-interruption control and requests while execution remains active. A blocked
  channel is a defect even if it eventually fits the outer ceiling or VM disposal
  succeeds; state a predeclared responsiveness oracle for the controlled fixture.
- Use deterministic HTTP/client controls to distinguish serial occupancy,
  continued guest work, relay close/half-close and host deadlines. A concurrent
  server may be an identified experimental control, never proof of cancellation;
  measure child survival separately. Use at most one instrumented clone.
- Check known child identities, retrieve partial evidence when possible, and
  preserve failed calls. The surviving controller bounds exact-child cleanup and
  disposes only its VM in `finally`; channel, cleanup and disposal have separate
  outcomes and any uncertain resource has an identity/remedy.
- Deliver a verified Taskfile diagnostic and
  `docs/experiments/exec-reliability-linux-disconnect-availability.md` with an
  old-path failure or explicit non-reproduction, controls and raw receipts,
  causal/transport limits, design constraints and resource audit.

## Notes

Abrupt client loss is separate from the future explicit cancellation protocol.
Do not choose cancel-on-disconnect, continued execution, a server replacement or
durable jobs here. Design must settle them from evidence. This Linux inquiry
makes no Windows reliability claim.

## Decisions (running log)

- **Boundary hypothesis, from source.** `agents/linux/testanyware_agent/server.py:123`
  serves with a plain `HTTPServer`, so one thread handles one request at a time.
  `handle_exec` blocks in `subprocess.run` until the guest timeout. The app relay
  (`cli-rs/crates/testanyware-app/src/streams.rs:253`) sends the end of the CLI
  bytes as a half-close, then resets the guest connection when the CLI is gone.
  The agent reads nothing during `subprocess.run`, so it cannot see the reset.
  The measurement tests this hypothesis. It does not assume it.
- **Two seams.** The deterministic controls run inside one fresh Linux container
  (the pinned k5 image, `--network none`, uid 1000). Client, agent and process
  table share one Linux kernel and one loopback, with no port proxy. Each case
  starts a fresh unchanged agent. The live run uses one fresh unchanged clone
  of `testanyware-golden-linux-24.04` through the installed CLI and app. No clone
  is instrumented.
- **Container cases.** `serial-control`, `serial-concurrent`, `serial-close`,
  `serial-reset`, `serial-half-close`, `host-deadline` (a 5 s client deadline on
  health during occupancy), and `threaded-reset`. `threaded-reset` serves the
  unchanged `AgentRequestHandler` from `ThreadingHTTPServer`. It is the
  identified experimental control `experimental: threading-server`. It is never
  evidence of cancellation.
- **Acceptance without instrumentation.** For a listening socket, the Linux
  `/proc/net/tcp` `rx_queue` field holds the accept-queue length. The container
  seam samples it every 50 ms. A queued follow-up raises it, and the accept of
  the agent lowers it. The exit of the payload shell marks the end of the
  original handler, because the handler kills the shell at the deadline.
- **Live burst reuse.** The live plan runs `disconnect_k13_burst.py` unchanged,
  with its `/tmp/taw-k13/MODE` guest paths. The payload is therefore the same as
  in the macOS run, byte for byte. The owned cleanup is
  `linux_k204.owned_cleanup` over the three child PID files.
- **Live handler end.** A serial agent answers no inspection while the handler
  runs, so the shell identity of k13 is not available. The handler-end
  reference is the exit of the original CLI in `concurrent`. In `term` it is
  the start plus the handler duration in `control`, labelled as an estimate.
- **Responsiveness oracle, frozen before measurement.** A follow-up request is
  *responsive* only when all three conditions hold:
  1. It ends with its expected outcome: exit 0, the expected JSON or bytes.
  2. Its latency is at most the latency of the same request in the control, plus 5 s.
  3. It does not start before the handler-end reference and end after it.
  A request with the expected outcome that fails condition 2 or 3 is a
  *blocked channel*, even inside its bound. A request with no answer inside its
  bound, or with another outcome, is a *failure*. Child survival, owned cleanup
  and disposal are three separate outcomes. A blocked channel does not decide
  them, and they do not decide the channel outcome.
- Freeze before measurement. The runner is `linux_k6_run.sh container|live OUT`, and the checker is `linux_k6_check.py container|live OUT`. The Taskfile entries are `diagnostic:linux-disconnect-{container,live,check,tools}`. The tools task passes ruff, pyright, shellcheck and 17 controls. Every measured run uses these files unchanged. The outer records compare the tool digests before and after each run.
- The container disposal bound is 40 s. The controller refuses a disposal budget above 41 s, because it adds a 4 s reserve to reach the 45 s ceiling.
- The development trials ran in the scratchpad and are not evidence. They gave the same shape as the expected finding. Serial occupancy blocks health for about 18 s in each serial case. The threading control answers in 2 ms. In every case the handler runs to the 20 s deadline, and the child survives it.
- The checker reads the agent-side socket rows from `/proc/net/tcp`. Unread request bytes on an accepted row place the wait in the accept queue. A follow-up row is reported with `waited_from` and `read_by`.
- Evidence goes under `experiments/exec-reliability/evidence/linux-disconnect-availability-k6/`, as `container-01` and `live-01`, with their `-outer` records.
- Live attempt `live-01` ran with the frozen tools. The controller verdict is True, and the clone was disposed. The check exited 1, so the attempt is kept as a failed sample. The raw records are sound. Two k13 rules do not hold on Linux:
  - The Linux agent's timeout reply carries no `timedOut` field. The CLI reports `"timed_out": false` with `exit_code -1` and `stderr "Process timed out"`. The k13 rule requires `timed_out: true`. The missing flag is a finding of this leaf.
  - On the live clone, `systemd --user` (pid 1128, a subreaper) adopts the orphaned `sleep 120`. The serial agent answers the first inspection only after the handler ends. That inspection records the adopter as the shell. k13 then classifies the handler as overran, because the late inspection sees that pid present.
- Correction r2 to `linux_k6_check.py`, made before `live-02`. The Linux live rule judges the original reply by the Linux shape: exit 0, `ok` true, `exit_code -1` and `stderr "Process timed out"`. It records `timed_out` as a finding. A shell sighting counts as alive only when its command names the mode directory. A sighting of another process at the recorded shell pid counts as gone, as does an absent shell or a missing shell pid with a present child. The rule records the child's parent and the adopter's command. Neither rule depends on a wanted outcome. `live-01` stays retained, and the report states the correction.
- Live run `live-02` ran with the r2 tools unchanged. The controller verdict is True, the check exits 0, the tool digests are equal and the clone was disposed. In `term`, the first inspection waited 18.067 s, which reproduces failure C. The other follow-ups answered in at most 0.023 s. In every mode the handler ran to the deadline, and `systemd --user` adopted the surviving child. The report is `docs/experiments/exec-reliability-linux-disconnect-availability.md`.
- An independent review leaf goes before the next live consumer, because later leaves build on this boundary finding.
