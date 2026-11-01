# diagnostic-foundation-k11


## Goal
Deliver the smallest reusable, independently bounded diagnostic controller and
resource ledger that the causal inquiries can use. Establish the available
public routes to identified baseline and isolated test builds before k3 starts.


## Context
- The root's common investigation contract and ADR-0004 govern every live route.
- `Taskfile.yml` has app/CLI routing fixtures and agent build/golden override
  recipes. Read their actual source and public help before selecting commands.
- `macos-completion-k3` still owns scoping unsafe `ProcessRunnerTests` cleanup;
  do not invoke `agent:macos:test` in this preparatory leaf.
## Done when
- Add a project-owned controller/ledger under
  `experiments/exec-reliability/` and a verified Taskfile self-check. It records
  unique run IDs, declared finite cases, argv/raw streams/status, monotonic
  phase timing, read-subject hashes before/after, and exact owned identities.
  It supplies independent bounds for calls, child interruption and teardown;
  the controller survives termination of its command CLI.
- Self-check with owned local subprocesses and fake public-interface peers:
  overlong call, TERM ignored then KILL, failed inspection/transfer/cleanup,
  hashing drift and uncertain disposal. Observe each forced failure, preserve
  its receipt, and verify that only ledger-owned resources are targeted.
  Test independent `finally` disposal even when another channel blocks; mock
  disposal proves controller behavior, not live VM disposal or production repair.
- Produce a per-OS route table for unchanged installed agents and explicit
  checkout-built agents/Host CLI/app. Record supported installation/transfer or
  uniquely named disposable-baseline recipes, exact provenance checks and
  teardown. Check app-socket occupancy and Local Network prerequisites before
  any live route. Do not assume golden creation is the only install route.
- Establish each route with only a predeclared minimal identity/health probe
  where permitted, or mark it unavailable with the exact missing prerequisite,
  affected conditions and concrete next action. No causal workload runs here.
  Retain stopped goldens unchanged, use VM ids through the app, and stop only
  a test app started by this run. Never quit the installed app or install over it
  without separate explicit owner authorization. If owner action is required,
  present the concrete recipe and trade-off; keep dependent work pending until
  answered or explicitly classify that condition unavailable without a pass claim.
- Publish `docs/experiments/exec-reliability-diagnostic-foundation.md` with
  controller invocation, forced-failure results, route table, provenance and
  residual-resource audit. Identify which later inquiries can run and which
  route facts remain unknown. If setup itself needs another session, decompose
  at a usable controller/route boundary rather than shifting hidden work to k3.
- Insert a specific `review-impl` before the next live sibling if the delivered
  controller is material implementation; let findings insert reconciliation
  before its consumers. The causal inquiries remain live on the clean branch.
## Notes
This controller observes existing interfaces. It defines no production execution
owner, cancellation API or terminal-record schema. Its raw envelope can retain
undecodable/contradictory bytes without interpreting a future lifecycle format.
Acceptance preparation later extends and validates it against the chosen
contract and all four corruption controls. No installed tool, retained golden
or unrelated guest may change to make a diagnostic route available.
