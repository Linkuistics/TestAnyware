# macos-exec-outcomes-k12


## Goal
Measure macOS execution outcome classification and exact capture through the
existing process, HTTP and Host CLI seams, without choosing the correction.


## Context
- Reuse the controller and route table from `diagnostic-foundation-k11` and
  ownership-scoped process fixtures from `macos-process-fixtures-k185`.
- `ProcessRunner.runShell`, macOS `handleExec`, shared/vendored execution
  types and Rust `AgentClient.exec`/HTTP transport fixtures are source leads.
- Compare the case matrix in `linux-timeout-lifecycle-k5`; no Linux result
  establishes a macOS outcome. Keep wait-race and inspection causes in k3/k4.
## Done when
- Freeze a finite local process/HTTP/Host CLI matrix: exit 0/23, signal death,
  forced spawn failure at the subprocess seam, deadline expiry, and partial
  stdout/stderr including trailing LF. Include the root's exact text/status
  byte oracle and transfer its immutable binary oracle where applicable.
- Record raw process status and termination reason, raw agent response and real
  Host CLI receipt separately, with exact bytes and phase timings. Force spawn
  failure only at a narrow existing boundary; label instrumentation explicitly.
  Unsupported representations and synthetic -1 remain observed defects rather
  than expected numeric guest outcomes. Do not infer timeout from stderr.
- Exercise the process boundary locally first and the real agent HTTP/client
  wire shape through existing fixtures. Use at most one unchanged and one
  instrumented macOS clone only if a fact requires the guest route. A missing
  live route stays unavailable under k11's table; fixtures prove only their seam.
- Compare actual timeout grace/kill/reap to <=2 s plus <=2 s; keep violations
  and failed inspection visible. Use independent whole-invocation bounds and
  positively identified processes. Do not widen the root's ceilings.
- Deliver verified Taskfile diagnostics, controlled old-path defect detectors
  and `docs/experiments/exec-reliability-macos-exec-outcomes.md` with per-case
  bytes/status, provenance, causal limits, design implications and resource audit.
  Ordinary checks stay green; known-failure diagnostics are explicit.
## Notes
No future terminal-record format or cancellation protocol is introduced here.
The inquiry measures existing behavior and hands the missing macOS evidence to
`lifecycle-evidence-k8`. Acceptance record-corruption checks belong after design.

## Decisions (running log)
- No clone. Each fact in Done-when is reachable on the host through an
  existing seam, so the guest route is not needed. The limit stays visible:
  the guest OS build and its launchd context are not measured.
- The matrix has eight frozen cases: `oracle-text`, `binary`, `exit-23-partial`,
  `signal-kill`, `exit-9`, `deadline`, `deadline-term-ignored` and
  `spawn-failure`. `exit-9` is the control that `signal-kill` must not equal.
- Five seams receive the same cases. `reference` is Python `subprocess` with
  `os.wait4`, the independent raw oracle. `process` is the unchanged
  `ProcessRunner.swift` built into a probe. `traced` is the same source built
  with `-DEXIT_TRACE`, which records Foundation status, termination reason,
  kernel exit status and phase times. `http` is the unchanged agent router on
  127.0.0.1, read as raw HTTP bytes. `cli` is the checkout CLI in text and JSON
  modes, routed through a test-mode app to that loopback agent.
- The loopback agent is a copy of `agents/macos` whose `main.swift` only binds
  127.0.0.1. The copy keeps `AgentServer.swift` and `ProcessRunner.swift`
  byte-identical, and the runner records both digests. The original `main.swift`
  binds 0.0.0.0 and drives Notification Center, so it does not run on the host.
- Spawn failure uses `RLIMIT_NPROC` of 1 on the process that calls
  `posix_spawn`. The label is `instrumentation: rlimit-nproc`. The user already
  owns more processes than the limit, so the next spawn fails with EAGAIN.
- Each seam is one controller `Call`, so the controller gives an independent
  whole-invocation bound and evidence storage. A seam script starts and stops
  its own agent and app, and identifies each by PID and start time.
- Expectations are the faithful outcome for each case, not source predictions.
  The checker labels each difference as an observed defect class. The old-path
  detector task exits 0 only when the frozen defect set is present, as k3 did.
- The runner task uses `uv run --locked python`, not `python -I`. Python 3.12
  with `-I` leaves the script directory out of `sys.path`, so the import of
  `controller` failed. The k195 runner uses the same form.
- The CLI text mode has no timeout flag. The checker records no flag for it and
  never reads a timeout from stderr. A deadline there is `deadline-not-flagged`.
- A completion that came back as a timeout is `completed-reported-timeout`. The
  detector accepts it in place of the deterministic defect of that case, because
  the k3 race replaces the stream and status of the case.
- Run `run-01` is the evidence. It ran once with no retry. The check passes, and
  the old-path detector exits 0. Three `cli-json` completions came back as 10 s
  timeouts. They stay in the evidence as observed.
