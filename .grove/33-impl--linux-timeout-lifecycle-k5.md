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

- Hash-compare retained source with the tested Linux agent. On one fresh
  unchanged clone freeze/run exact `sleep 120` and fork/wait controls with
  timeout 2 s, plus the retained complete fixture as its own case. Capture raw
  agent and real Host CLI receipts, host/guest status, partial bytes and known
  shell/child identities. Preserve every failed sample.
- Independently observe each recorded child after timeout; failed observation
  is unknown. Bound host calls by guest budget +20 s, ordinary observations by
  30 s, guest cleanup by 30 s and independent disposal by 45 s. Compare actual
  termination phases to <=2 s grace plus <=2 s kill/drain/reap even if violated.
- Exercise normal exit 0/23, signal death, forced spawn error at the subprocess
  seam, deadline expiry and partial stdout/stderr including trailing LF. Include
  the root's exact byte oracle. Keep unrepresentable outcomes visible; synthetic
  -1, empty capture or stderr text cannot substitute for numeric guest failure.
- Add deterministic process and real HTTP/Host CLI controls that isolate
  classification, capture and ownership separately. Use at most one instrumented
  clone if lower-level controls cannot settle a question. Identify old-path
  failing regressions without making the ordinary suite await a future repair.
- Deliver a verified Taskfile recipe and
  `docs/experiments/exec-reliability-linux-timeout-lifecycle.md` with per-case
  raw evidence, causal limits, regression candidates and full resource audit.
  Cleanup targets only positively identified owned processes/groups.

## Notes

A timeout field cannot establish cleanup, and a cooperative fixture cannot
prove arbitrary descendant containment. Client disconnect/request availability
belongs to `linux-disconnect-availability-k6`; explicit cancellation and the
production lifecycle are design work. Select neither here.
