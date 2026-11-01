# diagnostic-foundation-k16

**Integrates:** diagnostic-foundation-k15

## Goal

Triage the findings of review `diagnostic-foundation-k15` against the diagnostic
controller of `diagnostic-foundation-k11`. Apply the findings that are real
before a causal inquiry uses the controller.

## Context

- Read the findings in the review leaf for `diagnostic-foundation-k15`, as its
  commit records them. Its line numbers refer to producer commit `dfd48c9`.
- The implementation is under `experiments/exec-reliability/`. The durable
  handoff is `docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- Each finding has a recorded decision: applied, rejected with a reason, or
  moved to a named later leaf.
- Each applied controller change has a failure control. Show that control fail
  against the old behaviour before you credit its pass.
- A changed controller is a new condition. Retain the earlier self-check
  evidence unchanged, and record the new run under a new directory.
- The report states each changed limit and each route fact that remains open.

## Notes

Do not invoke `agent:macos:test`. The leaf `macos-completion-k3` owns the scope
of its unsafe cleanup. Do not quit or replace the installed app. A live guest probe
must follow the root brief's common investigation contract. Declare its bounds
before the run. Keep the historical 0/10 unavailable trial unchanged.

## Decisions (running log)

- Read review `diagnostic-foundation-k15` from commit `4bb47c05`, against the
  unchanged producer `dfd48c9e`. Tier 2 graph generation is
  `2026-11-01T04:03:20Z`; targeted discovery and source agree on the controller
  boundary. Graph trace contains unrelated same-name matches, so exact source
  determines the calls relied on. Coverage reports no recorded gaps on the
  existing evidence paths; the task body was read directly.
- Repair F1 with an explicit expected controller reason plus the existing exact
  host exit oracle. Default remains ordinary exit. Expected interruption or
  deadline may continue the same plan; observer/spawn errors remain failures.
  Resource actions must expect ordinary exit, so a timed-out disposal cannot
  become successful by declaration.
- Repair F3's filename collision and raised-I/O exception paths: reserve control
  receipt names before acquisition, catch OSError per action, retain failure in
  memory and attempt subsequent cleanup/disposal/inspection independently.
  Blocking evidence I/O remains outside the guarantee. Making teardown
  independent of a stalled filesystem needs a separately reviewed producer.
- Repair F9's missing-subject filtering, skipped-acquisition accounting and
  macOS docstring. Require a new explicit self-check output directory and retain
  checker stdout/stderr, statuses, per-test verdicts and subject digests.
  Preserve all existing evidence directories byte for byte.
- Implementation order: add failing controls and retain their old-condition
  output; repair the bounded paths; run fresh lint/type/test checks through the
  Taskfile; record all finding dispositions and place remaining producer work
  before causal consumers. No live probe or unsafe macOS suite in this session.
- The new controls failed against the unchanged controller in `k16-red-02`
  (eight failures) and `k16-red-03` (nine, adding preflight missing-subject
  preservation). `k16-red-01` is a retained checker-launch failure: resolving the
  venv Python symlink lost its installed pytest. It is not a controller control.
- `k16-green-01` passed 24 tests but failed strict typing of the runner's variable
  argv tuple lengths. A type annotation repairs that checker error.
  `k16-green-02` passes all 24 tests, lint, formatting and strict Pyright; subjects
  are unchanged before/after each run. All earlier conditions remain unchanged.
- F2 is real preparation work, moved to `diagnostic-routes-k19`. F3's blocking
  I/O architecture moves to `diagnostic-disposal-k18`; until corrected, a stalled
  evidence filesystem voids the disposal ceiling. F4, F5, remaining F6 controls
  and the complete F7 historical audit move to `diagnostic-evidence-k17`.
  These producers precede all causal consumers and must cut their own concrete
  reviews; design k18 also cuts its implementation/planning continuation.
- F8 is conservative rather than false-success behavior: retain uncertainty
  now; k19 must demonstrate an exact already-absent result and independent
  inventory oracle before broadening accepted disposal outcomes. F9's golden
  name guard also moves to k19. The task only checks NAME nonempty today;
  do not run it to obtain a new condition in this integration.
- Decline an in-session reader: the executable old-condition failures directly
  contest these repaired branches. The unresolved architecture and process
  guarantees have separate producer/review work; a fresh read here would not
  establish a stalled-storage or live-route result.
- Close F7 here rather than leave its historical audit to k17. New immutable
  audits cover each original uncertain entry and every integration fake-resource
  declaration. The final audit checks 46 fake-resource entries, including 15
  uncertain ledger entries across all conditions, and 189 recorded process IDs.
  It removes the final blocked-disposal marker; ps absence requires exit 1 and
  empty stdout AND stderr. No process is signaled. All read evidence digests
  match before/after. k17 now owns only audit of its own future samples.
- Final runner condition `k16-final-01` passes 24/24 tests, Ruff lint/format and
  strict Pyright. The report records every F1–F9 disposition, revised limits,
  route contradictions, exact artifact digests and unchanged historical trial.
  No guest, image, archive or test app was acquired; all observed owned fake
  markers and recorded local process IDs are absent in the final audit.
- Task guard verification exposed an unhelpful missing-OUT template error.
  The Taskfile now defaults an absent value to the empty string for validation
  and quoting. New condition `k16-final-02` passes all four checks and 24/24 tests
  with that final Taskfile digest. `k16-final-01` is retained unchanged.
  The final independent audit is `k16-audit-03`: 54 declared fake-resource
  entries, 18 preserved uncertain entries and 229 recorded process IDs,
  all exact marker paths/recorded PIDs absent at observation. This is a later
  audit condition, not a change to any earlier disposal result.
- Final verification checks each retained condition's artifact manifest and
  before/after subject equality, all audit input digests, and delivered source
  equality to `k16-final-02`. Missing OUT and reused OUT both reject before any
  checker or acquisition; raw guard logs are under `k16-task-guards-02`.
  VCS shows no changes in the original self-check, preflight, original audit or
  original index paths. No current live review names this integration producer,
  so no Creator line is due. The root still has live work; no parent closes.
