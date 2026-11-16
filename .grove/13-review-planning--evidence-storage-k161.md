# evidence-storage-k161

**Reviews:** evidence-storage-k158

## Goal

Review the plan that slices the evidence storage design into impl leaves.

## Context

The plan is the node `.grove/14-k162/`. Read its brief and its six task files.
The design is `docs/specs/diagnostic-evidence-storage.md`.

## Done when

- Each spec rule that needs code belongs to exactly one slice. Consider the
  channel contract, the phase bounds, the descriptor rules, the terminal
  outcome and the completion authority.
- Each slice leaves the suite green and `execute()` compatible with its callers.
- Each of the thirteen controls belongs to the slice that makes it pass.
- Each finding is in this file with the evidence for it.

## Notes

Specific doubts:

- The controls slice must fail each control on a build that has no writer and
  no writer command parameter. Check that each control can fail for the right
  reason on that build.
- The cutover slice runs with a writer whose close wait is not yet bounded.
  Check that this intermediate state cannot hang the suite.
- The completion authority changes in the writer slice, before `execute()`
  produces a close record. Check that `retention.py` and its tests stay green
  for run directories that `execute()` writes before the cutover.
