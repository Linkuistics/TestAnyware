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
