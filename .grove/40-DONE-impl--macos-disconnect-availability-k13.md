# macos-disconnect-availability-k13


## Goal
Account for macOS guest work and independent request responsiveness after
abrupt Host CLI interruption, separately from guest timeout and file consistency.


## Context
- Issue #14 failure C says both OSes left a known child alive. Failure D's
  `macos-candidate-02/after-abrupt-03-*` artifacts are later transfer evidence;
  they do not establish macOS disconnect cleanup or channel responsiveness.
- Reuse k11's controller/routes and k3/k4's safe process fixtures and reports.
- Match `linux-disconnect-availability-k6` comparison semantics and distinguish
  macOS `handleExec`/`ProcessRunner` from app/client close or half-close.
## Done when
- Freeze one unchanged macOS clone's exact issue #14 fork/wait payload with a
  20 s guest timeout, a no-interruption control and a finite concurrent-request
  control. Interrupt only the command CLI with SIGTERM at 2 s, allow <=2 s,
  then SIGKILL with <=2 s more if necessary. Preserve host signals/status and
  partial evidence while the outer controller survives.
- Immediately request known-child inspection, health and the immutable partial
  file through the public VM-id path, independently bounded at ordinary 30 s
  and transfer 150 s. Predeclare the controlled responsiveness oracle; record
  request acceptance/response and original-handler completion. A blocked channel
  is a defect even if it later responds within the outer ceiling.
- Measure child survival separately from handler/relay behavior using recorded
  identities. Failed inspection is unknown; failed downloads remain failures.
  Do not diagnose mutable-file corruption here: retain any such sample and
  hand it to `download-consistency-k7` rather than retrying it to success.
- Use deterministic HTTP/client controls and at most one instrumented macOS
  clone to distinguish continued execution, request availability, close/half-close
  propagation and deadlines. No clean timeout or Linux result substitutes for
  this inquiry. State a non-reproduction or unavailable phase explicitly.
- Deliver verified Taskfile diagnostics and
  `docs/experiments/exec-reliability-macos-disconnect-availability.md` with raw
  receipts, causal limits, design inputs and resource audit. Bound exact-child
  cleanup at 30 s and VM disposal independently at 45 s in `finally`, preserving
  uncertain identities/remedies. Reuse the common finite-run/isolation contract.
## Notes
Do not choose cancel-on-disconnect or continued execution here. Explicit guest
cancellation is a later independent design/acceptance case. File consistency,
ordinary completion and post-timeout inspection retain their own inquiries.

## Plan
1. Add the host seam `experiments/exec-reliability/disconnect_k13_seam.py`.
   It runs a loopback copy of the agent and a test-mode app.
   It records six cases with one tagged fork/wait payload each.
2. Add the host runner `disconnect_k13_host.py`. It builds the agent copy
   and runs the seam as one controller call.
3. Add the live probe `disconnect_k13.py` and its wrapper
   `disconnect_k13_burst.py`. One unchanged clone runs the frozen plan once.
4. Add `disconnect_k13_check.py`, `disconnect_k13_run.sh` and
   `test_disconnect_k13.py`. Add the Taskfile tasks.
5. Run the controls, the host run and the live run once each.
6. Write `docs/experiments/exec-reliability-macos-disconnect-availability.md`.

## Decisions (running log)
- The live path is CLI, then app socket, then app relay, then agent TCP.
  On a macOS host, `app_socket_for_host` always returns a socket.
- A SIGTERM ends the CLI with no handler. The app relay sees EOF and
  half-closes the guest stream. It then resets the stream when the client is
  gone. The host seam therefore records close, reset and half-close apart.
- The interrupt uses the controller interrupt path. `interrupt_after=2.0`
  signals only the CLI PID with TERM, then KILL, at the default 2 s bounds.
- The follow-up requests are separate controller calls. Their receipts share
  one monotonic clock with the interrupted call.
- The concurrent control needs overlap, so one wrapper call makes it.
  The wrapper sends the same three requests in the same order at 2 s.
- Responsiveness oracle, declared before any run: a follow-up request is
  responsive when three conditions hold. It ends with its expected outcome.
  Its response comes before the original handler ends. Its latency is at
  most its control latency plus 5 s. A request that answers later is a
  blocked channel. A request with no answer inside its bound is a failure.
- The live run has no direct handler signal. A parent shell that is present
  at the immediate inspection shows that the handler still runs. A parent
  that is absent at the late inspection shows that the handler ended.
- The host seam measures the handler end directly. The shell is a host
  process, and a half-closed connection still receives the response.
- No instrumented clone runs. Since v2.8.0, `ProcessRunner.swift` changed
  only inside `#if EXIT_TRACE`, and `AgentServer.swift` did not change.
  The default host build therefore runs the released handler code.
- The host payload keeps the issue shape but tags its sleep as
  `sleep 120.<digit><tag>`. A shared host path or a bare `sleep 120`
  could name a process that this run does not own.
- Development trials are not evidence. Trial 1 had one anomaly. In
  `cli-concurrent`, the PID file did not come within 5 s, and the CLI health
  took 6.2 s. Trial 2 and three latency probes did not show it again. The
  checker therefore records a missing identity as unknown, never as a pass.
  The report records the trial 1 anomaly as an open question.
- The seam runs with `python -E -s`, not `-I`. The `-I` flag removes the
  script directory from the import path, so the sibling modules fail.
- Exact expected outcomes, from trial 2. The original exec answers HTTP 200
  with `exitCode` -1 and `timedOut` true. The CLI prints `exit_code` -1,
  `timed_out` true and exits 0. A health request answers HTTP 200, or CLI
  exit 4 with `ok` true and `reachable` true, because the host denies
  accessibility. The ordinary exec prints `ordinary-ok` with exit code 0.
- Condition 2 of the oracle fails in the live mode only when a request
  provably spans the handler end. It began no later than a sighting of the
  shell alive, and it answered no earlier than a sighting of the shell gone.
  Condition 3 bounds every other hold. The checker judges a request that
  began after the handler end by conditions 1 and 3 only.
- The live inspection matches the shell by its mode directory in the command
  line. It records the shell in `shell.of.<child PID>`, and the checker
  rejects a role table whose child names another parent. Reason: dry run 2
  read a stale `shell.pid` from dry run 1 and judged a live handler as
  cancelled at 2.1 s.
- The mid and late inspections run on their own timer threads. Reason: in dry
  run 1, one 10 s reply pushed `inspect-mid` to 22 s, and the handler outcome
  became unknown.
- Dry run 1 and a development probe showed the k12 receipt
  `completed-reported-timeout` after a CLI TERM. The exec output was complete,
  but the reply said `timed_out` true after 10 s. Before the first TERM, 18
  short execs were fast. After it, slow replies came at irregular intervals,
  also in a later mode with no disconnect. Agent threads stayed at 23 to 27.
  These are development signals, not evidence.
- Both checkers classify that receipt as `completed-reported-timeout`. It is
  a finding, also in a control. An inspection with that reply and a complete
  role table still gives a sighting inside its request window.
- Host run `host-01` measured the receipt in the `cli-concurrent` control.
  Its first judgement (`check-01.json`, sha256 `1da69d92…4e95d`) had no class
  for it. The revised checker judged the same receipts again into
  `check.json`. No measurement ran again. The `cli-term` ordinary comparison
  stays unknown, because no sound control latency exists. I did not choose a
  substitute reference after seeing the data.
- The live checker falls back along the chain `term`, `concurrent`, `control`
  when a reference mode has no sound latency for a request. I declared this
  before the live run.
- Live run `live-01` ran once with no retry. Its verdict was true and its
  check had no problem. Every `term` follow-up was responsive, and both
  children ended with their handlers.
- The live owned cleanup printed `cleanup confirmed` and still reported a
  guest timeout after 9.117 s. The report records it as a second
  `completed-reported-timeout`. The cleanup and disposal stayed within 30 s
  and 45 s.
- I did not add an investigation leaf for that receipt. The
  `lifecycle-evidence-k8` leaf already decides whether a missing fact is
  load-bearing, and the report marks the mechanism as open.
- I inserted `macos-disconnect-availability-k202` as the review leaf, in the
  k12 pattern.
