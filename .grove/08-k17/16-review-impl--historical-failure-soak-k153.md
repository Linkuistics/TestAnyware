# historical-failure-soak-k153

**Reviews:** historical-failure-soak-k152

## Goal

Review the k152 soak instrumentation, plan, result and verdicts before the k17
whole-contract decision uses them.

## Context

Review the commit that retires historical-failure-soak-k152. Read that leaf's
done-when list and its running decision log in full.

Read the stamps in `experiments/exec-reliability/peer.py` and the prebuffer
fixture stamps in `experiments/exec-reliability/test_worker_capture.py`. Read
`timeline.py`, `soak.py`, `control.py` and `analyse.py` in
`experiments/exec-reliability/evidence/historical-soak-k152`.

Read `plan.json`, `report.md`, `verdicts.json` and `resource-audit.json` in the
same directory. Use the retained runs in `soak/` and `control/`. Read the k152
section in `docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- The review states whether each stamp records the event that its name claims.
- The review states whether the frozen control detects a missing or misordered field.
- The review states whether each recurrence predicate in `analyse.py` matches its historical symptom.
- The review states whether the soak ran its frozen plan once, without retry or extension.
- The review states whether each "did not recur" verdict follows from its exposure counts.
- The review states whether the run-42 mechanism follows from the retained timestamps.
- Each finding has a severity, a file and line, and a failure scenario.

## Notes

The soak audit missed the leaked worker child of run 42. The decision log and
`resource-audit.json` record the manual removal. The fixture repair belongs to
`worker-deadline-cleanup`. Do not rerun the soak to check a finding. k18 owns
stalled storage. No live guest, app or ten-clone acceptance claim follows.
