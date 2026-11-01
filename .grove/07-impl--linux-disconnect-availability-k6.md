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
