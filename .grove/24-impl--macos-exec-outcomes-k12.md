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
