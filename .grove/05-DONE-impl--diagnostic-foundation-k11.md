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

## Decisions (running log)

- Use a small Python standard-library controller at the existing subprocess
  boundary. A Python API lets causal fixtures declare their finite operations
  and exact disposal commands without introducing a second guest backend.
  File-backed raw streams avoid an unbounded pipe EOF wait after CLI death.
- Freeze the case order, budgets, subjects and ledger before executing calls.
  Each call gets a separate process session; interruption targets that recorded
  group. Cleanup, disposal and positive absence inspection have separate receipts.
  A failed inspection leaves disposal uncertain. Escaped descendants and crashes
  of the controller remain outside this diagnostic controller's guarantee.
- Implement and self-check the controller, then perform a finite read-only
  route preflight (installed help/version, public app status, VM and Tart
  inventories, source/build existence and signing identities). No guest boot,
  workload, app launch/quit, golden replacement or installation is authorized
  by this preflight. Missing live routes will be explicitly unavailable.
- Use macOS kqueue process-exit observation before reaping, so group signals
  cannot target a recycled leader identity. Python's macOS build lacks
  `os.waitid`. Treat observation errors as failed calls with bounded termination.
  Explicit CLI interruption signals its PID; an overlong local call signals
  its owned group. Neither action implements guest cancellation.
- The public preflight found installed 2.7.0 occupying the default socket,
  stopped baselines but no running guest, and no checkout-built executables.
  Preserve Doctor's Start at Login failure. Classify live build/guest routes as
  unavailable with concrete preparation steps rather than stop the installed
  app, change its configuration, or claim a new guest's health was measured.
- The final locked self-check passed all 13 tests, lint, formatting and strict
  typing. Frozen raw evidence retains expected failed operations, and a
  separate audit removed the remaining fake marker and found the recorded
  local process identities absent. No live VM/image/app was acquired. The
  durable handoff is `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- Insert a specific review before k3: the controller is material diagnostic
  implementation and its limits are load-bearing for every later inquiry.
  Do not run an in-session reviewer beside that scheduled review.

## Implementation plan

1. Add failure-first pytest checks and fake command peers under
   `experiments/exec-reliability/`; observe the missing controller failure.
2. Implement frozen declarations, bounded calls, raw receipts, subject hashes,
   exact resource ledger and independent finally disposal. Add the verified
   Taskfile self-check and preserve forced-failure evidence.
3. Read public help and relevant implementation, record read-only preflight
   receipts and per-OS route prerequisites in the durable report.
4. Verify the artifact, insert its specific review before k3, retire and seal
   this leaf with jj. The scheduled review owns adversarial verification.
