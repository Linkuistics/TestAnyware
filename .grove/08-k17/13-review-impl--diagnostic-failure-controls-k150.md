# diagnostic-failure-controls-k150

**Reviews:** diagnostic-failure-controls-k149

## Goal

Review the k149 runtime and test changes before k18 consumes the k17 process-control contract.

## Context

Review the commit that retires diagnostic-failure-controls-k149.
Read that leaf's done-when list and its running decision log in full.

Read the `_group_snapshot` grammar change and its comment in `experiments/exec-reliability/controller.py`.
Read the grammar rows in `experiments/exec-reliability/test_controller.py`.
Read the admission checks in `experiments/exec-reliability/test_worker_capture.py` and `experiments/exec-reliability/test_shutdown.py`.
Read the late-created identity fixture in `experiments/exec-reliability/test_shutdown.py`.

Read the k149 section and the k17 reconciliation in `docs/experiments/exec-reliability-diagnostic-foundation.md`.
Use `report.md`, `verdicts.json` and the retained runs in `experiments/exec-reliability/evidence/failure-controls-k149`.
An internal subagent review is recorded in the `review` section of `verdicts.json`. It does not replace this review.

## Done when

- The review states whether the `?E` grammar admits a member that can still run user code.
- The review states whether each admission assertion detects a late, stale or missing admission.
- The review states whether the late-created identity fixture still detects a dropped identity.
- The review states whether each historical class in `verdicts.json` follows from its retained controls.
- The review states whether the two real controls prove what the k17 reconciliation claims.
- Each finding has a severity, a file and line, and a failure scenario.
- When findings need action, an `integrate-review-impl` leaf exists for them.
- No live guest, app or acceptance claim follows from this review.

## Notes

k17 closes when this review retires with no open integration leaf. k18 owns stalled storage.
The UID-denial control needs a human owner with `sudo`. Do not rerun it without one.
