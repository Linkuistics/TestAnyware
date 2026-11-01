# bounded-inspection-k27

**Integrates:** bounded-inspection-k26

## Goal

Triage the findings of review bounded-inspection-k26 against producer
bounded-inspection-k24, then apply the real ones to the spec and its views
before bounded-inspection-k25 plans the implementation.

## Context

Read the findings from the review's own commit. Read k17's node brief, the
producer's commit and task file, `docs/specs/diagnostic-process-inspection.md`
and the three views under `docs/design/diagnostic-process-inspection/`. The
delivered controller is still k23-final-01. This step changes no code.

## Done when

- Each finding has a recorded disposition, with evidence for each rejection.
- Accepted changes land in the spec, the views and the foundation report
  together, and the three still agree.
- A design change that the review's alternatives do not settle becomes a new
  design and review chain ahead of k25, not an unreviewed rewrite here.
- k25, k21, k18 and the k17 brief read the reconciled contract.

## Notes

No live guest, app quit, unsafe process suite or acceptance claim. The design
remains unshipped until k25's implementation lands and is reviewed.

## Decisions (running log)

- Read all seven findings from review commit `12d0bdba`, against producer
  `53589d41`. The spec and views have no intervening changes. Tier 2 source
  checks use graph generation `2026-11-01T05:35:55Z`, exact source and complete
  targeted search/trace pages. Coverage records Mermaid gaps at ownership
  lines 1–13 and helper-start line 16. Direct reads cover those ranges.
- F1 is a real issue. A request dispatched just before T can retire a healthy
  worker. Select the review's first alternative: remove pre-escalation
  inspection and send group KILL at T without a table condition. Accept the
  full grace duration and the redundant signal to a zombie-only group.
  PID-only interruption retains conditional KILL. A late-grace leader-exit
  control must demonstrate that no inspection deadline falls at T.
- F2 states the fork contract incompletely. Add explicit thread and SIGCHLD
  preconditions, child signal/wakeup/descriptor reset, fresh child ownership
  state and `os._exit` on every child failure or return. Add inherited-state
  and worker-TERM controls. Python's documented fork and exit behaviour supports
  the finding, and tests call the controller inside pytest.
- F3 is a real trade-off. Drop the per-helper controller ACK. Keep creation
  intent, identity publication before gate release, token/expiry checks and
  receipt validation. Recording before exec adds no required guarantee when
  controller-crash recovery is excluded. Worker readiness remains acknowledged.
- F4 states the test seam incompletely. Freeze hold points and the helper
  executable in the bootstrap adapter. Separate deterministic helper KILL from
  actual setuid-ps KILL evidence. A genuine UID-based EPERM fixture needs an
  authorised owner with privilege. Its absence remains an itemised unavailable
  prerequisite, never a reason to acquire privilege during this session.
- F5 states the deadline and outcome contract incompletely. Define controller
  decision times, the non-failing zombie-only EPERM classification, final-query
  timing limits, fork-failure versus unknown-creation outcomes and every new
  plan bound. Preserve sticky failures and the existing shared ceilings.
- F6 states the scheduling contract incompletely. Require one controller
  scheduler for every wait phase, channels and worker timers. Existing `_wait`,
  `_terminate` loops and `Popen.wait` show that this needs explicit restructuring.
  Move subprocess-run stubs to adapter value cases in k25's implementation charter.
- F7 is a real artifact issue. Keep the spec and view manifest to the current
  contract. Move migration orders into k25 and keep delivery status in the
  foundation report and briefs. No new ADR is warranted: this remains a local,
  reversible design. These repairs use the review's alternatives and require
  neither a new design chain nor another in-session reviewer.
- The spec, three views, manifest, README, foundation report, root/k17 briefs
  and k25/k21/k18 consumers now agree. Helper creation/execution failure also
  retires the worker, including a timely fork error with no helper residual.
  No ADR changes or new task chain are needed. k25 and k21 remain live, so
  retirement of this leaf closes no parent.
- Verification rendered each of the three Mermaid views in Chrome and checked
  their deep links. Manifest IDs/source files and 51 local Markdown links and
  section targets passed direct checks. Before/after SHA-256 digests matched
  for the spec, visual README, manifest, three sources, viewer, foundation report
  and Taskfile. Mobile rendering was not tested. These document checks do not
  demonstrate runtime scheduling or process cleanup.
- The first preview attempt on port 8765 failed because another process held
  the port. It was left untouched. The Taskfile preview on port 18765 rendered
  successfully. The temporary browser tab was closed and its server PID 34622
  received TERM. Task reported child exit 143, and its tool session exited 201.
  No diagnostic workload, guest, golden, TestAnyware app or privileged fixture
  was acquired.
- Final document checks passed for all changed files: JSON, whitespace and local
  links, with unchanged per-file digests during validation. The diff contains
  documents only. Final graph metadata is stale for the edited files, so direct
  source/diff reads and rendered views supply the final evidence. There is no
  coverage-completeness claim. No live review names this integration as its
  producer, so retirement requires no Creator line.
