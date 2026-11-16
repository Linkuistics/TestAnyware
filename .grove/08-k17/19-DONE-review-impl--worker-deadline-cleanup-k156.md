# worker-deadline-cleanup-k156

**Reviews:** worker-deadline-cleanup-k154

## Goal

Review the k154 fixture cleanup and its controls before the k17 whole-contract
decision relies on the repaired verdict.

## Context

Review the commit that retires worker-deadline-cleanup-k154. Read that leaf's
done-when list and its running decision log in full.

Read `_remove_unreaped_child` and its callers in
`experiments/exec-reliability/test_bootstrap.py` and
`experiments/exec-reliability/test_worker_capture.py`. Read the `finally` block of
`test_worker_shutdown_deadline_closes_owner_operations`. Read
`stall_control.py` and `audit.py` in
`experiments/exec-reliability/evidence/worker-deadline-cleanup-k154`.

Use the retained runs in `runs/`, `removals/` and `resource-audit.json` in the
same directory. Read the case `host-stall-stale-kill-k152` in
`experiments/exec-reliability/evidence/historical-soak-k152/verdicts.json`. Read
the k154 section in `docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- The review states whether each fixture SIGKILL can reach only the exact child that its test created.
- The review states whether each repaired test still fails when the fixture removes the child.
- The review states whether `worker_capture.py` keeps its fail-closed rule after the deadline.
- The review states whether the stalled signal reproduces the run 42 condition.
- The review states whether the survey covers each fixture whose child waits on `signal.pause()`.
- The review states whether `resource-audit.json` names a removal for each child of each run.
- The review states whether the verdict case and the k17 lists follow from the controls.
- Each finding has a severity, a file and line, and a failure scenario.

## Notes

Three control defects and their retained runs are in the decision log. Do not
rerun the k152 soak to check a finding. The cause of the run 42 observation gap
stays unknown. k18 owns stalled storage. No live guest, app or ten-clone
acceptance claim follows.

## Decisions (running log)

- Reviewed producer commit `78c8fb4ef64a84e2c4cf26caa325bb74bb8c4dfd` by
  inspection only. No tests, builds, lint, formatters or controls were run, and
  no production or test source was edited. Read the retained receipts and
  compared their identities and hashes directly.
- The fixture repair and the red/green deadline controls support the narrow
  run 42 leak repair. The per-child resource audit does not meet the producer's
  acceptance: it omits known children and attributes some fixture removals to
  the runtime. R1 and R2 below require integration before relying on the whole
  k154 evidence claim.

## Review findings

All paths below are repository-relative. Evidence paths prefixed `k154/` mean
`experiments/exec-reliability/evidence/worker-deadline-cleanup-k154/`.

### R1 — medium: the resource inventory omits successfully removed children

Location:
`experiments/exec-reliability/evidence/worker-deadline-cleanup-k154/stall_control.py:181`
and
`experiments/exec-reliability/evidence/worker-deadline-cleanup-k154/audit.py:53`.

The control collects identities from deadline-audit receipts, the first stalled
signal, residual owner records and surviving child rows. It does not collect
identities from the other fixtures' receipts. A child reaped by another test
without reaching the stalled signal disappears from this inventory. The audit
then iterates only that incomplete inventory; its empty `unremoved` array cannot
establish that every created child has a removal record.

The retained final survey without stalls,
`k154/runs/pause-survey-nostall-final-20261116T222343-AC58EFB2/control.json`, has
170 test outcomes but only ten resources. Its receipts contain 170 distinct
child PIDs in `audit.pid`, `owner.pid` or `runtime.owner.pid`; 160 are absent
from the control and the resulting resource audit. For example,
`k154/runs/pause-survey-nostall-final-20261116T222343-AC58EFB2/receipts/persistent-startup-identity-1794828229500760000.json`
names PID 84367, reaped with return code -15, which is absent from the audit.
The final stalled survey likewise has at least 39 receipted child PIDs absent
from its 104-resource inventory, including PID 83686 in
`k154/runs/pause-survey-stall-final-20261116T222240-CA3911A9/receipts/persistent-close-denied-kill-1794828204047521000.json`.

This fails the explicit requirement to list each child and its removal in each
control run. It also makes the claim at
`docs/experiments/exec-reliability-diagnostic-foundation.md:2268` unsupported.
Integration needs a complete per-run child inventory reconciled with the
retained receipts, and must preserve any identities/removals that the retained
data cannot establish as unknown. A new soak is not needed to demonstrate this
finding.

### R2 — medium: an ECHILD observation is credited to the wrong cleanup actor

Location:
`experiments/exec-reliability/evidence/worker-deadline-cleanup-k154/audit.py:67`.

`state_after_tests == "reaped before audit"` proves only that the final control
cannot wait for that PID. When no receipt contains the literal `fixture_kill`
key, the audit labels the removal "runtime signal or child exit, reaped by the
runtime". Several existing fixtures remove children independently without that
key, so this branch misreports the actor.

For a concrete retained case, PID 83687 in
`k154/resource-audit.json:1793` is credited to the runtime.
`k154/runs/pause-survey-stall-final-20261116T222240-CA3911A9/receipts/persistent-close-audit-command-failure-1794828208075475000.json:2`
instead records an independent fixture reap with status 9. Its companion
`k154/runs/pause-survey-stall-final-20261116T222240-CA3911A9/receipts/persistent-close-command-failure-1794828208072355000.json:5`
records the same PID with `reaped: false` and no runtime signals before
independent cleanup. The source at
`experiments/exec-reliability/test_worker_capture.py:2760` shows this fixture
signals the pinned group and reaps the child itself when the runtime has not
reaped it. The final control's ECHILD observation does not turn that removal
into a runtime reap.

The actor matters here because the control distinguishes runtime cleanup from
fixture rescue after expired runtime authority. Integration needs to classify
removals from the actual per-fixture evidence, retaining an unknown actor when
the evidence does not decide it. This is separate from R1: the wrong actor is
reported even for a child already present in the inventory.

## Acceptance readback

- Exact-child safety: the new deadline-fixture SIGKILL follows independent
  `waitpid` polling that has not consumed this child's status. Its adapter either
  withholds real waits or records a successful late reap, which suppresses the
  SIGKILL. The two new `_remove_unreaped_child` copies use the owner of the
  fixture's direct child. In their current three callers no independent reaper
  consumes that child before this cleanup; runtime reaps set `owner.reaped` and
  suppress the helper. Thus the unreaped child pins the exact PID. These
  helpers are not authority to signal an arbitrary stale owner or an owner
  whose child has been reaped elsewhere. The other surveyed PID cleanups retain
  their own child status; the existing persistent-close group cleanup has a
  pinned session leader and its pause service creates no descendants. The
  control's own SIGKILL first establishes an unreaped direct child with real
  `waitpid`. No unrelated PID or unpinned group is justified by these paths.
- Failure visibility: the deadline fixture asserts `fixture_kill` is absent;
  the bootstrap callers require `owner.reaped` and `removal is None`; the
  persistent startup caller either fails its existing reaped assertion or the
  post-cleanup `removal is None` assertion. Fixture rescue cannot make these
  repaired tests pass. Retained red/green and all-ten deadline controls agree.
- Runtime cutoff: `_signal` still refuses a signal at or after the owner's
  shutdown deadline. The current runtime and all three frozen input copies
  have SHA256
  `05dc982b0f4c27a381a092e83099fce87fec632e8f6fbbbcbbb6ca70275e1191`.
- Run 42 reproduction: the wrapper delays entry to the real `_signal` until
  the finite deadline has passed. Both stale-kill variants then retain a live
  child and owner record in the red control; the green fixtures remove them
  and still fail. This reproduces the deadline/no-signal/leak condition, not
  the unknown cause of the historical observation gap.
- Survey coverage: the retained final surveys include all nine pause-fixture
  test functions and their 170 parameter variants across `test_bootstrap.py`,
  `test_worker_capture.py` and `test_shutdown.py`. Source inspection confirms
  their cleanup paths, including the persistent-close owner-pop ordering.
  Survey pass means no observed child or owner residue; its `--survey` mode
  permits intentionally passing/non-signalling variants. It does not establish
  a complete child inventory or removal attribution (R1 and R2).
- Resource audit: it does not name a justified removal for each child of each
  run. The retained final controls and audit have matching recorded control
  hashes, so R1 and R2 concern the committed evidence, not file drift. The
  retained ten controls each report matching before/after input hashes.
- Verdict and handoff: the narrow `fixture_defect` classification and repaired
  run 42 fixture leak follow from the opposed stale-kill controls. The broader
  each-child/each-actor claim must be repaired or bounded visibly through
  integration. The k17 whole-contract result correctly remains false; five
  nonrecurring cases remain unexplained, the run 42 gap cause stays unknown,
  k18 owns stalled storage, and ten-clone acceptance remains 0/10 unavailable.

## Evidence limits and handoff

Graph project
`Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14` reports
generation `2026-11-15T02:19:52Z`. Relevant live source has changed metadata and
the new evidence is not tracked by that generation. Graph discovery did not
resolve the new helper; coverage checks reported no recorded parse gaps, which
is not proof of completeness. This review therefore relies on direct current
source reads and retained-file inspection for its material claims.

R1 and R2 belong to the next `integrate-review-impl` sibling under k17. This
placement keeps the same acceptance scope and gives the integration session
the review commit before another consumer can move its evidence coordinates.
