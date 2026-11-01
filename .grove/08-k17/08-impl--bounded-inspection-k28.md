# bounded-inspection-k28

## Goal

Replace synchronous process inspection through `execute` with the reviewed
worker boundary and one controller scheduler, retaining a checkable failed
receipt while owned-resource safety actions continue on their original bounds.

## Context

Read `docs/specs/diagnostic-process-inspection.md` in full, the bounded-inspection
section of `docs/experiments/exec-reliability-diagnostic-foundation.md`, and
bounded-inspection-k27's seven dispositions in commit `dd264245`. Planner
bounded-inspection-k25 records the increment boundary. k23-final-01 remains the
delivered source condition until this change is measured and reviewed.

Own `experiments/exec-reliability/controller.py`, a narrow inspection module if
needed, `peer.py` and any owned local helper fixtures, `test_controller.py`,
`self_check.py`, `preflight.py` where it declares controller inputs,
`pyproject.toml`'s explicit type-check/test subjects, concrete `Taskfile.yml`
recipes, new retained control/audit directories and the durable foundation
report. Preserve all earlier evidence directories byte for byte. Keep old
receipts readable; extend immutable `Call`/`Plan`/`Resource` declarations only
as the reviewed contract requires and update their existing callers together.

## Done when

- One scheduler serves every workload wait, termination phase, nonblocking
  leader reap, resource cleanup/disposal/inspection, both channels and worker
  shutdown timer. Replace private `_wait`/`_terminate` wait loops and the
  leader's blocking `Popen.wait` with bounded steps. Check due safety actions
  before messages; consume at most 64 KiB per turn across channels. Preserve
  acquisition intent, stop-on-first-failure, exact stdout checks and reverse
  acquired-resource teardown, including independent disposal after failed
  cleanup. An open teardown command cannot defer worker TERM/KILL/reap.
- Before resource acquisition, bootstrap one worker in its own session with
  readiness acknowledgement. Confirm native and Python single-threaded state
  and SIGCHLD=SIG_DFL before every worker/helper fork. Implement blocked-signal
  reset, default child handlers, cleared wakeup descriptor, descriptor allowlist,
  fresh ownership state and `os._exit` for every child return/failure. Unsupported
  hosts fail preparation. Before confirmed session readiness, cleanup is pinned
  PID-only. Every later preparation exit also accounts for the started worker.
- The worker alone creates, execs, captures and reaps its helpers. Freeze the
  absolute helper executable and private hold points at bootstrap. Publish
  creation intent, then pinned helper identity before releasing its exec gate;
  no per-helper controller ACK is required. Enforce token/expiry/EOF checks.
  A timely explicit fork error retains errno without inventing a helper;
  silence after intent becomes an unknown creation attempt at deadline or
  worker exit. Late identities refine cleanup obligations only.
- Use bounded nonblocking control and result channels: 1 MiB per message,
  256 KiB combined raw table output and 4096 parsed rows. Control identities
  cannot queue behind table output. Validate all run/worker/request/workload/
  epoch identities, complete framing, unique terminal result, table and ordered
  dispatch/query/receipt times. Reject duplicates, stale/late/partial/conflicting
  results and overflow without turning partial output into a valid table.
- The controller alone owns workload PID/group signals, including signal 0,
  and workload reaping. Pin the leader until closing every signal/probe entry
  point. Group grace has no query; attempt group KILL at T even after leader
  exit. PID-only interruption keeps conditional PID KILL and never widens scope.
  Follow the spec's fixed T/K/R definitions for all three exit paths, at most
  one final query, cutoff closure and nonblocking reap within K. No late turn
  moves a deadline. Record the observation interval separately from signal 0.
- Retain live, zombie-only, genuine permission failure/unknown and invalid
  observation outcomes with their stated consequences. A live final group
  fails after normal exit or expected PID-only interruption, with members and
  revalidation remedy. Zombie-only EPERM does not alone fail or explain an
  earlier denied attempt. Missing/failed inspection never proves absence.
- Any inspection creation/execution, protocol or deadline failure is sticky
  and retires the worker permanently. No replacement, query retry or queued
  final query behind a stalled request. Later workload calls remain unreached;
  resource safety/public absence commands are still attempted with their local
  inspection unavailable and disposal certainty failed. Keep worker/helper,
  unknown creation, workload leader and observed-member residuals distinct.
  Retain strong ownership until bounded shutdown settles or records residuals;
  helper and worker reaps cannot substitute for workload reap.
- Declare and validate bootstrap readiness (at most 1 s), bootstrap-failure
  TERM/KILL-reap, worker-failure TERM/KILL-reap and orderly-close grace/KILL-reap
  (each at most 2 s) before freezing a run. Worker cleanup runs concurrently
  with safety work. Orderly close has one grace, and an already retiring worker
  keeps its original clock. No hidden context-manager, destructor, exit-hook,
  channel drain or join wait may block return. Keep workload grace and shared
  kill/drain/reap at most 2 s each, guest cleanup 30 s and disposal 45 s.
- Demonstrate every control in the spec's required-controls table through the
  real controller, using the inventory below. Retain per-case verdicts, failed
  samples and independent exact-identity cleanup audits. A rejected damaged
  receipt stays failed even when its checker passes.
- Extend self-check lint/hash/test/type-check coverage for every new runtime,
  fixture and checker subject, including the explicit pyright include list and
  any test-file selection. Reuse `mise exec -- task diagnostic:self-check`
  with `OUT=<new directory>` and `diagnostic:review-controls` as applicable;
  verify concrete new Taskfile recipes before relying on them. Keep checker
  timeout, unreaped-process and wrong-stdout rejection controls. Freeze source,
  configuration, lockfile, interpreter, helper and checker identities, finite
  case order and all bounds. Finish edits before each run; hash every read
  subject before and after and retain raw streams, timing and per-case outcomes
  in new directories. Never increase a ceiling to conceal a stalled control.
- Update the foundation report with the delivered commit/source condition,
  receipts, unavailable controls, limits and independent itemized resource
  audit. Supply k21 immutable observation/residual inputs without a hash cycle;
  stream/result identity and surviving-writer late-change controls remain k21.
  Give k18 the in-memory safety boundary; stalled evidence open/read/write/hash
  and teardown evidence descriptors remain k18. No publication ACK permits
  helper exec, workload signalling, reap or disposal.
- As the final producer act, insert a specific `review-impl` with the bare
  `bounded-inspection` stem before retained-evidence-k21, naming this handle in
  `**Reviews:**`. Require an actual-runtime trace of helper creation, result
  drains, waits, destructor/shutdown hooks, workload and worker signal authority,
  concurrent teardown timers and residual auditing. Findings reconcile before
  k21; create no empty integration. If the runtime defeats the contract,
  externalise the precise counterexample to design ahead of its consumer,
  rather than extend timeouts, add threads/workers or widen signal scope.

## Required control inventory

Use the same hashed worker source with holds disabled for ordinary conditions.
Record each fault adapter configuration and helper identity separately. Bound
the complete test invocation with an independent outer controller, which also
audits known fixture children and helpers without using the faulted worker.

- Real delayed worker/helper beyond request deadline: observe target KILL/reap
  attempts and marker-producing disposal on unchanged deadlines. Later release
  cannot repair the receipt. Exercise holds before fork, between fork and
  publication and before exec; distinguish explicit fork failure from unknown
  creation. Audit every published helper and keep unpublished creation uncertain.
- Inherited host handlers, exit hook, wakeup and evidence descriptors: TERM a
  healthy worker while a workload lives. Independently reject worker-origin
  workload signals, evidence writes and host hooks. Reject extra native/Python
  threads and non-default SIGCHLD before fork.
- Deterministic owned slow helper: actual KILL, then a hold at wait return;
  retain PID, real signal result and independent helper/worker accounting while
  safety proceeds. Separately label a no-completion wait adapter as simulated
  unreapability, never an actual uninterruptible kernel task.
- Actual identified setuid-ps KILL is a separate timing-sensitive control.
  Record real/effective UIDs, actual signal and reap evidence; exit before the
  attempt leaves delivery unproved. Genuine UID-based EPERM requires a
  separately authorised privileged fixture owner for creation and cleanup.
  Do not acquire privilege here. Missing timing/owner prerequisites produce
  unavailable receipts naming prerequisites, intended denial/cleanup method,
  known identities and every unreached field/operation. Neither another helper
  nor injected errno satisfies these real controls. Real denial plus missing
  table must yield `permission_unknown`; retain zombie-only EPERM separately
  without claiming its cause. Missing real controls prevent a full guarantee.
- Move foreign-PGID, ps usage/timeout and missing-leader subprocess-run stubs
  to adapter value cases. Keep them distinct from real permission evidence.
  Add wrong epoch/identity, duplicate/conflicting terminals, partial framing,
  stderr, raw/row/message overflow, backpressure and post-cutoff delivery.
  Independent safety markers must still appear; later valid bytes stay rejected.
- Retain real early-reap and stale-callback rejection before any later signal
  0/TERM/KILL; audit workload and worker authority separately. Keep opposed
  PID/group scope, conditional PID KILL, normal/PID-only live-child failure,
  controller-survival and checker-rejection controls. Audit each known child;
  a missing leader or malformed selector is never an absence oracle.
- Exit within the last 30 ms of group grace: no query before KILL at T and
  final budget remains available. Exercise leader exit observed after K minus
  R and still-exiting members in the sole final query; retain both failures
  without retries. Mutate the shared cap/overrun predicate and show rejection.
- Hold a teardown command open across worker TERM, KILL and reap deadlines;
  show actions retain their original deadlines. Exercise bootstrap failure,
  post-bootstrap preparation exits and healthy orderly close under the declared
  invocation bounds, including already-retiring-worker cleanup.

## Notes

No live guest, installed-app change, unsafe macOS process suite or acceptance
claim. Workload process creation, stalled evidence storage, arbitrary escaped
descendants and controller-crash recovery remain outside this increment's
demonstrated guarantee. Do not deliberately induce an uninterruptible kernel
task. k17 stays open through reviewed implementation and k21; unfulfilled real
controls must remain visible when its whole contract is checked. If this
increment proves too large, decompose only at independently useful, green
behaviour boundaries and never claim the complete guarantee from a partial step.
