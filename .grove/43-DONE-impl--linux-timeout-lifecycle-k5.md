# linux-timeout-lifecycle-k5


## Goal

Reproduce Linux timeout misclassification, partial capture loss and owned-child
survival at their responsible boundaries, with independently useful diagnostics.



## Context

- Issue #14 failure B, retained `diagnosis-linux-01/` downloaded sources and
  original `fixture/r5/guest.sh` in the downstream evidence.
- Linux `handle_exec` and server route.
- Rust `ExecResult`, `AgentClient.exec`, `endpoint_integration` and
  `transport_seam`. Observe optional `timedOut`
  defaulting at each boundary rather than infer timeout from stderr.

## Done when

- Hash-compare the retained source with the tested Linux agent.
- Exercise normal exit 0/23, signal death, forced spawn error at the subprocess
  seam, deadline expiry and partial stdout/stderr including trailing LF. Include
  the root's exact byte oracle. Keep unrepresentable outcomes visible. Synthetic
  -1, empty capture or stderr text cannot substitute for numeric guest failure.
- Add deterministic process and real HTTP/Host CLI controls that isolate
  classification, capture and ownership separately. Run the exact `sleep 120`
  and fork/wait controls at these seams. Observe each recorded child after the
  timeout. Compare the termination phases with <=2 s grace plus <=2 s
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
belongs to `linux-disconnect-availability-k6`. Explicit cancellation and the
production lifecycle are design work. Select neither here.

Outcome: `run-03` under
`experiments/exec-reliability/evidence/linux-timeout-lifecycle-k5` is complete
and sound. Both checker modes exit 0. `run-01` and `run-02` stay as the refused
and the failed attempts. Each refreeze is its own commit before the next run.
The report is `docs/experiments/exec-reliability-linux-timeout-lifecycle.md`.

Decisions:

- The leaf runs the unchanged agent source in containers of one pinned image,
  because no clone is in scope. Leaf k204 owns the clone.
- The spawn failure sets `RLIMIT_NPROC` on the agent after it answers. A
  container that starts with the limit cannot `execve` its first process.
- A process is owned when the fresh case container did not list it before the
  case, by PID and start ticks. Cleanup kills only those processes.
