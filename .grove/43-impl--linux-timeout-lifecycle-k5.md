# linux-timeout-lifecycle-k5


## Goal

Reproduce Linux timeout misclassification, partial capture loss and owned-child
survival at their responsible boundaries, with independently useful diagnostics.



## Context

- Issue #14 failure B, retained `diagnosis-linux-01/` downloaded sources and
  original `fixture/r5/guest.sh` in the downstream evidence.
- Linux `handle_exec` and server route; Rust `ExecResult`, `AgentClient.exec`,
  `endpoint_integration` and `transport_seam`. Observe optional `timedOut`
  defaulting at each boundary rather than infer timeout from stderr.

## Done when

- Hash-compare the retained source with the tested Linux agent.
- Exercise normal exit 0/23, signal death, forced spawn error at the subprocess
  seam, deadline expiry and partial stdout/stderr including trailing LF. Include
  the root's exact byte oracle. Keep unrepresentable outcomes visible; synthetic
  -1, empty capture or stderr text cannot substitute for numeric guest failure.
- Add deterministic process and real HTTP/Host CLI controls that isolate
  classification, capture and ownership separately. Run the exact `sleep 120`
  and fork/wait controls at these seams, observe each recorded child after the
  timeout, and compare the termination phases with <=2 s grace plus <=2 s
  kill/drain/reap. Identify old-path failing regressions without making the
  ordinary suite await a future repair.
- Deliver a verified Taskfile recipe and
  `docs/experiments/exec-reliability-linux-timeout-lifecycle.md` with per-case
  raw evidence, causal limits, regression candidates and the resource audit of
  the seams. Cleanup targets only positively identified owned processes/groups.

## Notes

The live clone run moved to the sibling `linux-timeout-live-k204`, because the
whole leaf does not fit one session. That leaf owns the fresh-clone controls,
the retained fixture case, the live child observations, the instrumented clone
and the live resource audit.

A timeout field cannot establish cleanup, and a cooperative fixture cannot
prove arbitrary descendant containment. Client disconnect/request availability
belongs to `linux-disconnect-availability-k6`; explicit cancellation and the
production lifecycle are design work. Select neither here.
