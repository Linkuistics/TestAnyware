# bounded-inspection-k28 — brief

## Goal

Replace synchronous process inspection through `execute` with the reviewed
worker boundary and one controller scheduler, retaining a checkable failed
receipt while owned-resource safety actions continue on their original bounds.

## Context

Read `docs/specs/diagnostic-process-inspection.md` in full, the bounded-inspection
section of `docs/experiments/exec-reliability-diagnostic-foundation.md`, and
bounded-inspection-k27's seven dispositions in commit `dd264245`. Planner
bounded-inspection-k25 records the increment boundary. The delivered initial
admission slice is k32-final-01, following k29 and its review integration k32.
The complete worker/scheduler boundary remains complete-worker-scheduler-k91's
work after helper-fork-error-k72 reviews k69. Later bounded slices have landed
without establishing the persistent worker/shared scheduler guarantee; read the
current foundation report for their measured limits.

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

## Historical decisions (running log)

The entries below record the original implementation interruption and early
slices. Their source conditions are historical, not current stop instructions.
Current routing and promoted acceptance requirements follow under Decomposition.

- Keep the end-to-end increment selected by k25. The reviewed spec and plan
  settle the design and test seams. Implement inline in the isolated jj workspace.
  Controller scheduling and workload authority stay in `controller.py`; a private
  `inspection.py` owns worker transport and helper mechanics. Add failing controls,
  implement the boundary, migrate inspection stubs to value cases, freeze and run
  Taskfile checks, audit owned identities, update the handoff, and cut the required
  review before retirement. No in-session reviewer is added beside that review.
- Tier 2 discovery uses graph generation `2026-11-01T05:48:26Z`. Complete targeted
  controller search and execute trace identify synchronous inspection and blocking
  reap; cross-language heuristic edges are not evidence. Runtime/config coverage
  matches with no recorded gap. Changed brief/report metadata and the newly
  retired planner require direct reads, performed here.
- The owner requested a stop for Grove reconfiguration during implementation.
  Leave this leaf live and preserve the open jj change; do not retire, cut its
  review or seal it as a completed producer. `inspection.py` is an unfinished,
  unintegrated worker/protocol draft. `test_inspection.py` starts the real-controller
  controls. The first test was observed failing because `controller.InspectionConfig`
  does not yet exist. Neither `controller.py` nor the runner/configuration has
  changed, and no new worker/helper or resource fixture was launched. The
  delivered runtime condition remains k23-final-01.
- At interruption, both new Python files compile. Targeted Ruff check fails
  on formatting/import order (19 findings); no Taskfile measurement or type
  check has run. These checks are exploratory tool output, not retained control
  evidence or a repaired-boundary claim. On resumption, review the draft against
  the complete spec before integrating it: scheduler/workload authority, bounded
  retained protocol state, late identity reconciliation, terminal/close ordering,
  helper cleanup accounting and fault adapters are still unfinished. Then add
  the remaining test-first controls, runner/hash/type-check subjects and frozen
  evidence/audits before the required review. All earlier evidence stays intact.

## Notes

Fork-admission-k29 and later bounded slices preserve useful checked behaviors,
not the complete guarantee. Read current producer/review evidence for the slice
being used. No live guest, installed-app change, unsafe macOS process suite or
acceptance claim is authorized here. Workload creation, stalled evidence storage,
arbitrary escaped descendants and controller-crash recovery remain outside the
demonstrated guarantee. Do not deliberately induce an uninterruptible kernel
task. k17 remains open through reviewed implementation and k21; k18 separately
owns storage isolation. Missing real controls stay visible at whole-contract
acceptance. Further splits must deliver independently useful green behavior and
preserve every remaining requirement.

## Decomposition and promoted contract

This node earns its level because the helper-owning worker, bounded transport and
controller scheduler share one safety contract and real fault-control inventory
that other k17 tasks do not need. Its joint completion condition is the complete
Done when and control inventory above, followed by full runtime review and any
necessary reconciliation before k21. Its sessions are ordered flat siblings;
implementation repairs and reviews add no new context level.

The owner approved archiving continuation briefs k30, k34, k38, k40, k42, k44 and
k48 without certifying their unfinished aggregates. Their original text and
decision logs remain in [Grove brief history](../../../docs/planning/grove-history/README.md).
Their acceptance now belongs here and in complete-worker-scheduler-k88; historical references to those
handles route through that index. In addition to every requirement above:

- Preserve k33/k36 group KILL ordering, k37 strict table acceptance and k39's
  exact-PID WNOHANG reap until K, no destructor polling and strong workload owner
  after failed reap. The inspection cutoff is K-min(100 ms,A/4). No late turn
  extends authority or deadlines; reject stale callbacks.
- Preserve k41's sole final query after last signal and observed leader exit,
  no retry after live/unknown, and post-KILL EPERM classification. Preserve
  k43/k46 sticky attempted-query failure through safety commands.
  `GroupObservation.query_attempted` separates a skipped final query (which
  fails its call but permits later independent safety inspection) from a failed
  helper (which disables later local tables and probes). Valid live observations
  remain termination failures, rather than inspector failures.
- Preserve reviewed k47/k50 bounded capture/reap, exact helper identity and
  distinct retained residual owners. Reconcile the former untrusted draft with
  the actual runtime specification; its historical snapshot is not delivered code.
  Per-query worker isolation, raw helper fork/reset, gated publication, native
  admission repair and explicit fork-error receipts are bounded slices only.
- Keep real helper KILL and held wait return, simulated unreapability, and
  unavailable genuine UID denial/setuid-ps timing controls distinct. Freeze
  every read subject and bound, audit fixture/helper/worker/workload/member and
  unknown-creation obligations independently, and preserve prior evidence bytes.
- k72 reviews k69 before k75 relies on explicit fork failure. k75 retains the
  full remaining runtime charter, every required control and final review. That
  future review is **Reviews: bounded-inspection-k28**, carrying the former k30
  aggregate obligation; require actual runtime ownership/waits/destructors/
  signal authority/concurrency/full-inventory traces. Insert it before k21 and
  reconcile actionable findings there, with no empty integration.

The complete contract remains unmet. Neither archiving continuation levels nor
local passing slices closes this node, k17 or the root. Retained-file identity
remains k21, storage stalls remain k18, and ten-clone acceptance stays 0/10
unavailable. Historical slice decisions remain in their unchanged task bodies
and the archived briefs; they do not require rereading seven continuation levels
to find the current contract.

- 2026-11-15: k71 owns only cleanup-only late identity retention; k75 carries the entire remaining worker/scheduler contract and aggregate acceptance/review. This flat split ships independently useful behavior and closes no aggregate ancestor.

### k85 workload termination cutoff slice

The full aggregate continues in flat sibling complete-worker-scheduler-k88, including the persistent pre-resource worker, common scheduler, every outstanding control and final aggregate review. k85 only closes signal/exit-observation admission inside `_terminate` at the original deadline and rejects late observer returns. See experiments/exec-reliability/evidence/termination-cutoff-k85/report.md. Its separate review does not replace the required final bounded-inspection review. `_wait`, group query/probe admission, scheduler unification and all remaining waits retain their aggregate obligations. The failed full-1 real fixture observations are retained and remain unresolved; a later full pass is no repair claim. Aggregate ancestors remain open; ten-clone acceptance is 0/10 unavailable.

The k89 findings are reconciled by k90: signal authority closes at K-R (not K), and signal receipts share the admission sample. See experiments/exec-reliability/evidence/termination-cutoff-k90/report.md for red/green evidence and exact limits. k85’s six mutations cover only three deadline comparisons/equality variants. The expired-deadline first nonblocking reap attempt and all other aggregate requirements remain in complete-worker-scheduler-k88; k28/k17 stay open.

## Current decomposition

The first expired-deadline leader reap is k88's independently verifiable slice. complete-worker-scheduler-k91 retains the entire original runtime charter and all controls. Aggregate acceptance and final bounded-inspection review remain here; k28/k17 stay open.

### k121 review integration handoff

k120 F1 is repaired by the real empty-open control channel regression; M1 and
M3 survive the old 37 controls but fail the new open case (38 other cases pass).
The unchanged runtime passes all 39 expanded controls. F2 remains an explicit
trade-off: M2 survives, and the conservative guard stays. F3 now distinguishes
timely failure admission from real EOF framing within the existing reserve.
See `experiments/exec-reliability/evidence/fork-authority-k121/report.md`.

The one full sample is red: 855 passed, with
`test_worker_shutdown_deadline_closes_owner_operations[stale-term-pid]` failing
on a WNOHANG observation 20.291 microseconds after its deadline. The exact child
was independently reaped; raw runtime and audit receipts remain in
`experiments/exec-reliability/evidence/fork-authority-k121/candidate-full/`.
This failure joins k119's shared deadline obligations without replacing k116's
separate historical deferred-reap failure. Full-run Pyright and the first
static-only followup also exposed auditor-only errors; both failures and tool
versions are retained. Final static checks pass from the frozen-input cwd.
No repeated full run repaired the red sample. k119 still owns the shared
scheduler/teardown overlap and final k28 review; aggregate acceptance remains
0/10, unavailable.

### k122 workload-wait turn handoff

k122 delivers only incremental active-workload wait turns, used by the existing synchronous consumer. The complete aggregate charter and historical log now reside in flat sibling complete-worker-scheduler-k124, superseding earlier continuation-owner pointers above. The separate seam review precedes k124. See experiments/exec-reliability/evidence/workload-turns-k122/report.md: 18 focused passes, six rejected mutations, 866 full/static passes, 60 new child absence journals and unchanged prior evidence. Persistent failure cleanup and close remain synchronous; actual scheduler/teardown overlap, retained intermittent failures, complete real controls, interleaved comparisons and final k28 review remain open. No ancestor closes; guest acceptance is 0/10 unavailable.

### k132 readiness-wake handoff

k132 adds optional pipe interests to the common driver and publishes bounded query/retirement interests; it does not establish concurrent production owners. The intact aggregate charter and every historical handoff are now in flat sibling complete-worker-scheduler-k134, superseding older continuation pointers above. Independent readiness review complete-worker-scheduler-k135 precedes it. See experiments/exec-reliability/evidence/readiness-wakes-k132/report.md for nine new controls, 106 focused passes twice, six rejecting mutants twice, two final 896-test candidate passes, corrected frozen statics, verified hashes/94 fixture absence audits and 232,416 unchanged prior paths. Retain the original three parent timing failures, recurring expired-request residual_remedy None, original inference errors and repaired auditor import failure. The comparison gate remains rejected; clean candidates do not resolve parent failures. Actual shared scheduling and teardown overlap, every remaining control, interleaved comparisons against the completed scheduler and final review-impl of bounded-inspection-k28 before k21 remain open. k28/k17 stay open; guest acceptance remains 0/10 unavailable.

### k134 startup-turn slice and aggregate ownership

k134 removes only the private startup-readiness select loop by using bounded common-driver callbacks. Worker.start remains a synchronous compatibility owner. The entire prior aggregate charter and history moved intact to flat sibling complete-worker-scheduler-k137 before k134 was narrowed; k137 supersedes the continuation pointer immediately above. Its shared production scheduler, teardown overlap, remaining controls, historical failures and final full-runtime review of bounded-inspection-k28 remain binding. Read experiments/exec-reliability/evidence/startup-turns-k134/report.md and the separate startup review before relying on this slice. k28/k17 remain open, k18 retains storage work, and guest acceptance remains 0/10 unavailable.

### k139 startup frame control integration

Review F1 is repaired by fourteen real-child controls and twelve exact frozen mutants, each rejected by a new control. Focused results pass twice; the second full sample passes 921/921, but the first retains a shutdown WNOHANG admission 347 microseconds after its cutoff (`blocking-0-denied-kill`). The full comparison gate remains rejected and k137 carries that observation beside all prior failures. Runtime source is unchanged. The nesting guard uses an explicit stdlib Python-scanner seam because tested native decoder depths within the frame cap did not exhaust; the real-cutoff control deliberately delays a real pipe read. Neither seam claims natural blocking/native exhaustion. See `experiments/exec-reliability/evidence/startup-frame-controls-k139/report.md` for all retained failures, 306 journaled fixture absence audits and 264,064 unchanged prior paths. k137's entire aggregate charter, final k28 review and 0/10 unavailable guest acceptance remain binding; k28/k17 stay open.
